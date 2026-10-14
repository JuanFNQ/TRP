using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Text;
using BepInEx;
using BepInEx.Configuration;
using Comfort.Common;
using EFT;
using EFT.Animations;
using EFT.InventoryLogic;
using HarmonyLib;
using UnityEngine;

namespace Stlkreft.Oracle
{
    /// <summary>
    /// Records what the game's own controller does under scripted input.
    ///
    /// In a raid, stand on open level ground and press the run key (the open bracket, `[`).
    /// Every scenario in the plugin's `scenarios` folder is played in turn
    /// with the player's real input replaced by the script, and one trace
    /// per scenario is written to the `traces` folder: a header line, then
    /// one JSON object per frame.
    /// </summary>
    [BepInPlugin("stlkreft.oracle", "stlkreft oracle", Version)]
    public class Plugin : BaseUnityPlugin
    {
        /// Written into every trace, so that a reader can tell which
        /// fields to expect.
        private const string Version = "0.7.0";

        /// Seconds of stillness before each scenario so that one does not
        /// run into the next.
        private const float Settle = 2f;

        internal static Plugin Instance;

        private ConfigEntry<KeyCode> _runKey;
        private ConfigEntry<bool> _returnToStart;
        /// Where the player stood and looked when the run key was pressed.
        private Vector3 _start;
        private Vector2 _startLook;
        private bool _haveStart;
        private string _folder;
        private List<Scenario> _queue = new List<Scenario>();
        private Scenario _current;
        /// The script's own clock, which stands still at a shut gate.
        private float _time;
        /// The seconds since the scenario began, gates and all.
        private float _real;
        /// The gates gone through; whether the clock stands at one, for
        /// how long it has, and how long until that gate's key is next
        /// pressed.
        private int _passed;
        private bool _shut;
        private float _shutFor;
        private float _untilPress;
        /// This frame: the gate the clock stands at (or -1), and the key
        /// that gate presses (or null).
        private int _gate = -1;
        private string _gatePress;
        private float _settle;
        private StreamWriter _trace;
        private int _frame;
        private readonly Dictionary<string, bool> _held = new Dictionary<string, bool>();
        private readonly HashSet<int> _pressed = new HashSet<int>();
        private string _status = "";
        private Vector3 _origin;

        internal bool Running => _current != null && _settle <= 0f;

        /// A recording made by hand (0.6.0): no script, the player's own
        /// keys and mouse, a frame written for every frame drawn until the
        /// key is pressed again. For what no script can do: opening the
        /// inventory, searching a container, working a door, picking a
        /// thing up, changing what is in the hands.
        private bool _free;
        private string _freeName;
        private ConfigEntry<KeyCode> _freeKey;

        /// The key that switches the player between its own eyes and how
        /// others see it (0.7.0): the game's own switch on a player
        /// (`Player.PointOfView`), which every bot is made with and which
        /// no key of the release build reaches. A recording made after it
        /// is one of a character seen from outside: its body's own pose,
        /// the weapon on the body's weapon bone, the weapon's cut-down
        /// motion. The camera stays where the eyes were, so the picture is
        /// the inside of the head; the key again puts everything back.
        private ConfigEntry<KeyCode> _povKey;

        private void Awake()
        {
            Instance = this;
            _folder = Path.GetDirectoryName(Info.Location);
            // The setting was "RunKey" with F9 up to 0.3.0. A saved setting outlives a
            // changed default, so the new key goes under a new name.
            _runKey = Config.Bind("General", "StartKey", KeyCode.LeftBracket, "Plays every scenario and records a trace of each.");
            _returnToStart = Config.Bind("General", "ReturnToStart", true,
                "Puts the player back where the run key was pressed, looking the same way, before each scenario, so that all of them run over the same ground.");
            _freeKey = Config.Bind("General", "FreeKey", KeyCode.RightBracket,
                "Starts a recording made by hand, with the keys and the mouse left to the player, and ends it when pressed again.");
            _povKey = Config.Bind("General", "PointOfViewKey", KeyCode.Backslash,
                "Switches the player between its own eyes and how others see it, and back. Press it before a recording to record a character seen from outside.");
            new Harmony("stlkreft.oracle").PatchAll(typeof(AxesPatch));
            Logger.LogInfo("stlkreft oracle loaded; scenarios in " + Path.Combine(_folder, "scenarios"));
        }

        private static Player LocalPlayer()
        {
            var world = Singleton<GameWorld>.Instantiated ? Singleton<GameWorld>.Instance : null;
            return world != null ? world.MainPlayer : null;
        }

        private void Update()
        {
            var player = LocalPlayer();
            if (player == null)
            {
                Abort("no player");
                return;
            }
            // Between recordings only, or in one made by hand: a script's
            // traces are each of one point of view from end to end.
            if (_current == null && _queue.Count == 0 && Input.GetKeyDown(_povKey.Value))
            {
                bool own = player.PointOfView == EPointOfView.FirstPerson;
                player.PointOfView = own ? EPointOfView.ThirdPerson : EPointOfView.FirstPerson;
                _status = own ? "the player is now as others see it (the key again for its own eyes)" : "the player is seen with its own eyes again";
            }
            if (_current == null && _queue.Count == 0 && Input.GetKeyDown(_freeKey.Value))
            {
                if (_free) EndFree(); else BeginFree(player);
            }
            if (_free)
            {
                // The clock of a recording made by hand is the engine's.
                _real += Time.deltaTime;
                _time = _real;
                _status = string.Format(CultureInfo.InvariantCulture, "recording by hand  {0:0.0} s  (the key again ends it)", _real);
                return;
            }
            if (_current == null && _queue.Count == 0 && Input.GetKeyDown(_runKey.Value))
            {
                _haveStart = false;
                LoadScenarios();
            }
            if (_current == null && _queue.Count > 0)
            {
                Begin(player);
            }
            if (_current == null)
            {
                return;
            }
            if (_settle > 0f)
            {
                _settle -= Time.deltaTime;
                _status = "settling before " + _current.Name;
                if (_settle <= 0f)
                {
                    _origin = player.Transform.position;
                    // Every scenario starts upright at the top of the speed
                    // wheel, whatever the last one left.
                    player.ChangeSpeed(1f);
                    player.ChangePose(1f);
                    WriteHeader(player);
                }
                return;
            }

            Tick(player);
            ApplyCommands(player);
            _status = string.Format(CultureInfo.InvariantCulture, "{0}  {1:0.0} / {2:0.0} s{3}  ({4} more)",
                _current.Name, _time, _current.Duration, _gate >= 0 ? "  waiting on the legs" : "", _queue.Count);
            if (_time >= _current.Duration)
            {
                End(player);
            }
        }

        /// Moves the script's clock on by this frame, or holds it at a gate
        /// that is still shut: the same steps as `Clock::frame` in
        /// crates/trp-sim/src/script.rs, which the scenarios are tried
        /// with before they are recorded.
        private void Tick(Player player)
        {
            float dt = Time.deltaTime;
            _real += dt;
            _gate = -1;
            _gatePress = null;
            if (_passed >= _current.Gates.Count || _time + dt < _current.Gates[_passed].At)
            {
                _time += dt;
                return;
            }
            var gate = _current.Gates[_passed];
            _time = gate.At;
            _gate = _passed;
            if (_shut)
            {
                _shutFor += dt;
                _untilPress -= dt;
            }
            else
            {
                _shut = true;
                _shutFor = 0f;
                _untilPress = 0f;
            }
            float legs = player.Physical.Stamina.Current;
            bool open = gate.LegsBelow.HasValue ? legs < gate.LegsBelow.Value : legs >= gate.LegsFrom;
            if (open || _shutFor >= gate.Timeout)
            {
                _shut = false;
                _passed++;
                return;
            }
            if (!string.IsNullOrEmpty(gate.Press) && _untilPress <= 0f)
            {
                _untilPress = gate.Every;
                _gatePress = gate.Press;
            }
        }

        private int _writtenFrame = -1;

        private void OnEnable()
        {
            Camera.onPreCull += OnPreCull;
        }

        private void OnDisable()
        {
            Camera.onPreCull -= OnPreCull;
        }

        /// Runs as the first camera is about to draw: after every script
        /// has moved the player and the weapon for this frame, so the
        /// values written are the ones drawn. A plain LateUpdate here can
        /// run before the game's own and see the weapon bone unturned.
        private void OnPreCull(Camera camera)
        {
            if (Time.frameCount == _writtenFrame)
            {
                return;
            }
            var player = LocalPlayer();
            if (player != null && (Running || _free) && _trace != null)
            {
                _writtenFrame = Time.frameCount;
                WriteFrame(player);
            }
        }

        private void BeginFree(Player player)
        {
            _freeName = "free-" + DateTime.Now.ToString("yyyyMMdd-HHmmss", CultureInfo.InvariantCulture);
            _thing = null;
            // What is carried and the weapon's parameters are written afresh.
            (_carriedSum, _lootSum, _boundWas, _inHandsWas, _parametersOf) = (0, 0, null, null, null);
            _time = 0f;
            _real = 0f;
            _passed = 0;
            _shut = false;
            _gate = -1;
            _gatePress = null;
            _frame = 0;
            _held.Clear();
            _pressed.Clear();
            _origin = player.Transform.position;
            var dir = Path.Combine(_folder, "traces");
            Directory.CreateDirectory(dir);
            _trace = new StreamWriter(Path.Combine(dir, _freeName + ".jsonl"), false, new UTF8Encoding(false));
            _free = true;
            WriteHeader(player);
        }

        private void EndFree()
        {
            _free = false;
            _trace?.Dispose();
            _trace = null;
            _status = "recording by hand ended: " + _freeName + ".jsonl in " + Path.Combine(_folder, "traces");
        }

        private void OnGUI()
        {
            if (!string.IsNullOrEmpty(_status))
            {
                GUI.Label(new Rect(20, 20, 900, 30), "stlkreft oracle: " + _status);
            }
        }

        private void LoadScenarios()
        {
            var dir = Path.Combine(_folder, "scenarios");
            if (!Directory.Exists(dir))
            {
                _status = "no scenarios folder at " + dir;
                return;
            }
            var files = Directory.GetFiles(dir, "*.json");
            Array.Sort(files, StringComparer.Ordinal);
            foreach (var file in files)
            {
                try
                {
                    _queue.Add(Scenario.Load(file));
                }
                catch (Exception e)
                {
                    Logger.LogError("scenario " + file + ": " + e.Message);
                }
            }
            _status = _queue.Count + " scenarios queued";
        }

        private void Begin(Player player)
        {
            _current = _queue[0];
            _queue.RemoveAt(0);
            _thing = null;
            // What is carried and the weapon's parameters are written afresh.
            (_carriedSum, _lootSum, _boundWas, _inHandsWas, _parametersOf) = (0, 0, null, null, null);
            _time = 0f;
            _real = 0f;
            _passed = 0;
            _shut = false;
            _gate = -1;
            _gatePress = null;
            _frame = 0;
            _settle = Settle;
            _held.Clear();
            _pressed.Clear();
            // A scenario that ended lying down must not leave the next one
            // there. Getting up takes about a second of the settling time.
            if (player.MovementContext.IsInPronePose)
            {
                player.ToggleProne();
            }
            // Every scenario starts from the same spot, looking the same
            // way, with the body squared up to the view. The first one
            // fixes the spot.
            if (!_haveStart)
            {
                _start = player.Transform.position;
                _startLook = player.Rotation;
                _haveStart = true;
            }
            else if (_returnToStart.Value)
            {
                player.Teleport(_start);
                player.Rotation = _startLook;
            }
            player.MovementContext.ApplyRotation(Quaternion.Euler(0f, _startLook.x, 0f));
            var dir = Path.Combine(_folder, "traces");
            Directory.CreateDirectory(dir);
            _trace = new StreamWriter(Path.Combine(dir, _current.Name + ".jsonl"), false, new UTF8Encoding(false));
        }

        private void End(Player player)
        {
            ReleaseAll(player);
            _trace?.Dispose();
            _trace = null;
            _current = null;
            if (_queue.Count == 0)
            {
                _status = "done; traces written to " + Path.Combine(_folder, "traces");
            }
        }

        private void Abort(string why)
        {
            if (_current != null || _queue.Count > 0)
            {
                _trace?.Dispose();
                _trace = null;
                _current = null;
                _queue.Clear();
                _status = "stopped: " + why;
            }
        }

        // ------------------------------------------------------------ input

        /// Movement and look for the current frame, as the game's own
        /// input translator would have received them.
        internal void FillAxes(float[] axes)
        {
            float key(string name) => _current.IsHeld(name, _time) ? 1f : 0f;
            _current.TurnRate(_time, out var yaw, out var pitch);
            for (int i = 0; i < axes.Length; i++)
            {
                axes[i] = 0f;
            }
            axes[0] = key("d") - key("a");
            axes[1] = key("w") - key("s");
            axes[2] = yaw * Time.deltaTime;
            axes[3] = pitch * Time.deltaTime;
            // What the script asked the view to turn this frame, before the
            // game's sensitivity and the movement state's own limits.
            _askedTurn = new Vector2(axes[2], axes[3]);
        }

        private Vector2 _askedTurn;

        private bool Edge(string key, out bool down)
        {
            down = _current.IsHeld(key, _time);
            _held.TryGetValue(key, out var was);
            _held[key] = down;
            return down != was;
        }

        private void ApplyCommands(Player player)
        {
            var firearm = player.HandsController as Player.FirearmController;
            if (Edge("shift", out var sprint))
            {
                if (sprint) player.ToggleSprint(); else player.EnableSprint(false);
            }
            if (Edge("e", out var right))
            {
                if (right) player.ToggleLean(1f); else player.Straighten(1f);
            }
            if (Edge("q", out var left))
            {
                if (left) player.ToggleLean(-1f); else player.Straighten(-1f);
            }
            // The step to the side, held: standing it is the sidestep,
            // lying down the roll.
            if (Edge("step_right", out var stepRight))
            {
                if (stepRight) player.ToggleStep(1); else player.ReturnFromStep(1);
            }
            if (Edge("step_left", out var stepLeft))
            {
                if (stepLeft) player.ToggleStep(-1); else player.ReturnFromStep(-1);
            }
            // The vault key, held: the game's own command for it is one
            // call that switches the per-frame try on, and off again.
            if (Edge("vault", out _)) player.Vaulting();
            if (firearm != null)
            {
                if (Edge("aim", out var aim)) firearm.SetAim(aim);
                if (Edge("fire", out var fire)) firearm.SetTriggerPressed(fire);
            }

            for (int i = 0; i < _current.Presses.Count; i++)
            {
                var press = _current.Presses[i];
                if (_time < press.At || !_pressed.Add(i))
                {
                    continue;
                }
                PressKey(player, firearm, press.Key);
            }
            // The key of the gate the clock stands at, at that gate's spacing.
            if (_gatePress != null)
            {
                PressKey(player, firearm, _gatePress);
            }
        }

        private void PressKey(Player player, Player.FirearmController firearm, string key)
        {
            switch (key)
            {
                case "jump": player.Jump(); break;
                case "crouch": player.TogglePose(); break;
                case "prone": player.ToggleProne(); break;
                // What the game's own reload key does: it looks for a
                // magazine within reach and starts the reload with it.
                case "reload": if (firearm != null) new FirearmHandsInputTranslator(player, firearm).Reload(); break;
                case "wheel_up": player.ChangeSpeed(EFTHardSettings.Instance.DELTA_SPEED); break;
                case "wheel_down": player.ChangeSpeed(-EFTHardSettings.Instance.DELTA_SPEED); break;
                case "stance_up": player.ChangePose(EFTHardSettings.Instance.DELTA_LEVEL); break;
                case "stance_down": player.ChangePose(-EFTHardSettings.Instance.DELTA_LEVEL); break;
                case "auto": if (firearm != null) firearm.ChangeFireMode(EFT.InventoryLogic.Weapon.EFireMode.fullauto); break;
                case "single": if (firearm != null) firearm.ChangeFireMode(EFT.InventoryLogic.Weapon.EFireMode.single); break;
                // A shotgun's second mode (0.6.2): the M3 is pumped in
                // `single` and loads itself in this one.
                case "semiauto": if (firearm != null) firearm.ChangeFireMode(EFT.InventoryLogic.Weapon.EFireMode.semiauto); break;
                // The game's looks at the weapon, as its keys start them.
                case "check_magazine": if (firearm != null) firearm.CheckAmmo(); break;
                case "check_chamber": if (firearm != null) firearm.CheckChamber(); break;
                default: Logger.LogWarning("unknown press " + key); break;
            }
        }

        private void ReleaseAll(Player player)
        {
            var firearm = player.HandsController as Player.FirearmController;
            if (_held.TryGetValue("shift", out var sprint) && sprint) player.EnableSprint(false);
            if (_held.TryGetValue("e", out var right) && right) player.Straighten(1f);
            if (_held.TryGetValue("q", out var left) && left) player.Straighten(-1f);
            if (_held.TryGetValue("step_right", out var stepRight) && stepRight) player.ReturnFromStep(1);
            if (_held.TryGetValue("step_left", out var stepLeft) && stepLeft) player.ReturnFromStep(-1);
            if (_held.TryGetValue("vault", out var vault) && vault) player.Vaulting();
            if (firearm != null)
            {
                if (_held.TryGetValue("aim", out var aim) && aim) firearm.SetAim(false);
                if (_held.TryGetValue("fire", out var fire) && fire) firearm.SetTriggerPressed(false);
            }
            _held.Clear();
        }

        // ----------------------------------------------------------- output

        private static string N(float v)
        {
            if (float.IsNaN(v) || float.IsInfinity(v))
            {
                return "null";
            }
            return v.ToString("R", CultureInfo.InvariantCulture);
        }

        private static string V(Vector2 v) => "[" + N(v.x) + "," + N(v.y) + "]";
        private static string V(Vector3 v) => "[" + N(v.x) + "," + N(v.y) + "," + N(v.z) + "]";
        private static string Q(Quaternion q) => "[" + N(q.x) + "," + N(q.y) + "," + N(q.z) + "," + N(q.w) + "]";
        private static string B(bool b) => b ? "true" : "false";

        /// Which state one layer of an animator is in and how far through
        /// it, and the same for the state it is blending into, if any.
        ///
        /// A state is written as the low 24 bits of the hash of its own
        /// name, which a reader holding numbers as single floats keeps
        /// exactly; the time is in plays of the state, so it passes 1 on a
        /// state that loops.
        private static void AppendAnimator(StringBuilder sb, string name, IAnimator animator, int layer)
        {
            if (animator == null)
            {
                return;
            }
            var current = animator.GetCurrentAnimatorStateInfo(layer);
            sb.Append(",\"").Append(name).Append("_anim\":").Append(current.shortNameHash & 0xFFFFFF);
            sb.Append(",\"").Append(name).Append("_anim_time\":").Append(N(current.normalizedTime));
            var blending = animator.IsInTransition(layer);
            sb.Append(",\"").Append(name).Append("_blending\":").Append(B(blending));
            if (blending)
            {
                var next = animator.GetNextAnimatorStateInfo(layer);
                sb.Append(",\"").Append(name).Append("_next\":").Append(next.shortNameHash & 0xFFFFFF);
                sb.Append(",\"").Append(name).Append("_next_time\":").Append(N(next.normalizedTime));
            }
        }

        private void WriteHeader(Player player)
        {
            var sb = new StringBuilder(512);
            var mc = player.MovementContext;
            var pwa = player.ProceduralWeaponAnimation;
            var firearm = player.HandsController as Player.FirearmController;
            var cam = Camera.main;
            sb.Append("{\"scenario\":\"").Append(_free ? _freeName : _current.Name).Append('"');
            sb.Append(",\"by_hand\":").Append(B(_free));
            sb.Append(",\"recorder\":\"").Append(Version).Append('"');
            sb.Append(",\"game\":\"").Append(Application.version).Append('"');
            sb.Append(",\"fixed_dt\":").Append(N(Time.fixedDeltaTime));
            sb.Append(",\"gravity\":").Append(V(Physics.gravity));
            sb.Append(",\"max_speed\":").Append(N(mc.MaxSpeed));
            sb.Append(",\"inertia\":").Append(N(player.Physical.Inertia));
            sb.Append(",\"overweight\":").Append(N(player.Physical.Overweight));
            sb.Append(",\"walk_inertia\":").Append(N(mc.WalkInertia));
            sb.Append(",\"trunk_limit\":").Append(N(mc.TrunkRotationLimit));
            if (cam != null)
            {
                sb.Append(",\"fov\":").Append(N(cam.fieldOfView));
                sb.Append(",\"near\":").Append(N(cam.nearClipPlane));
            }
            if (firearm != null && firearm.Item != null)
            {
                sb.Append(",\"weapon\":\"").Append(firearm.Item.TemplateId.ToString()).Append('"');
                sb.Append(",\"weapon_weight\":").Append(N(firearm.Item.TotalWeight));
                // The chambered round: its recoil percentage scales every shot.
                var chambers = firearm.Item.Chambers;
                if (chambers != null && chambers.Length > 0 && chambers[0].ContainedItem != null)
                {
                    sb.Append(",\"ammo\":\"").Append(chambers[0].ContainedItem.TemplateId.ToString()).Append('"');
                }
                sb.Append(",\"ergonomics\":").Append(N(firearm.TotalErgonomics));
                sb.Append(",\"ergonomic_weight\":").Append(N(firearm.ErgonomicWeight));
                // How well the character knows this weapon: from level 2 a
                // reload keeps the sights up.
                sb.Append(",\"mastering\":").Append(firearm.CurrentMasteringLevel);
                // The length of the line the game casts along the weapon to
                // feel for obstacles (0.4.3). It is measured as the weapon
                // is drawn, so it is not quite the weapon's at rest.
                sb.Append(",\"weapon_length\":").Append(N(firearm.WeaponLn));
            }
            if (pwa != null)
            {
                sb.Append(",\"tactical_reload_stiffness\":").Append(V(pwa.TacticalReloadStiffnes));
                sb.Append(",\"tactical_reload_position\":").Append(N(pwa.TacticalReloadPosition));
                sb.Append(",\"weapon_flip_speed\":").Append(N(pwa.WeaponFlipSpeed));
            }
            if (pwa != null && pwa.HandsContainer != null)
            {
                sb.Append(",\"rotation_center\":").Append(V(pwa.HandsContainer.RotationCenter));
                sb.Append(",\"recoil_pivot\":").Append(V(pwa.HandsContainer.RecoilPivot));
                sb.Append(",\"camera_offset\":").Append(V(pwa.HandsContainer.CameraOffset));
                sb.Append(",\"far_aim_plane\":").Append(N(pwa.FarAimPlane));
                sb.Append(",\"aiming_speed\":").Append(N(pwa.AimingSpeed));
            }
            AppendBodyParts(sb, player);
            AppendInventory(sb, player);
            sb.Append('}');
            _trace.WriteLine(sb.ToString());
        }

        // What the character carries, as the game has it (0.5.0): the sum
        // the controller goes by, the same with Strength at its elite
        // level, and every item of the equipment with what it is in, the
        // slot or grid it is in there, its cell and turn in a grid, how
        // many are in its stack, its size in cells with its parts on and
        // its weight with all that is in it. The first entry is the
        // equipment itself.
        private static void AppendInventory(StringBuilder sb, Player player)
        {
            try
            {
                var inventory = player.Inventory;
                if (inventory == null || inventory.Equipment == null)
                {
                    return;
                }
                sb.Append(",\"inventory_weight\":").Append(N(inventory.TotalWeight.Value));
                sb.Append(",\"inventory_weight_elite\":").Append(N(inventory.TotalWeightEliteSkill.Value));
                sb.Append(",\"equipment\":[");
                var first = true;
                foreach (var item in inventory.Equipment.GetAllItems())
                {
                    if (!first)
                    {
                        sb.Append(',');
                    }
                    first = false;
                    sb.Append("{\"id\":\"").Append(item.Id).Append('"');
                    sb.Append(",\"tpl\":\"").Append(item.TemplateId.ToString()).Append('"');
                    var address = item.CurrentAddress;
                    if (address != null && address.Container != null && item != inventory.Equipment)
                    {
                        var holder = address.Container.ParentItem;
                        if (holder != null)
                        {
                            sb.Append(",\"in\":\"").Append(holder.Id).Append('"');
                        }
                        sb.Append(",\"at\":\"").Append(address.Container.ID).Append('"');
                        var cell = address as GridItemAddress;
                        if (cell != null)
                        {
                            sb.Append(",\"x\":").Append(cell.LocationInGrid.x);
                            sb.Append(",\"y\":").Append(cell.LocationInGrid.y);
                            sb.Append(",\"r\":").Append(cell.LocationInGrid.r == ItemRotation.Vertical ? 1 : 0);
                        }
                    }
                    sb.Append(",\"n\":").Append(item.StackObjectsCount);
                    var size = item.CalculateCellSize();
                    sb.Append(",\"w\":").Append(size.X).Append(",\"h\":").Append(size.Y);
                    sb.Append(",\"kg\":").Append(N(item.TotalWeight));
                    sb.Append('}');
                }
                sb.Append(']');
            }
            catch (Exception e)
            {
                // The trace is worth more than this one record.
                sb.Append(",\"inventory_error\":\"").Append(e.GetType().Name).Append('"');
            }
        }

        private void WriteFrame(Player player)
        {
            var sb = new StringBuilder(1024);
            var mc = player.MovementContext;
            var pwa = player.ProceduralWeaponAnimation;
            var root = player.Transform;
            // The seconds since the scenario began, and the script's own
            // time, which is behind them by as long as its gates were shut.
            sb.Append("{\"t\":").Append(N(_real));
            sb.Append(",\"script_t\":").Append(N(_time));
            if (_gate >= 0)
            {
                sb.Append(",\"gate\":").Append(_gate);
            }
            if (_gatePress != null)
            {
                sb.Append(",\"gate_press\":true");
            }
            sb.Append(",\"dt\":").Append(N(Time.deltaTime));
            // The frame's length as the player's own code was given it: the
            // game times the player by a stopwatch in whole milliseconds.
            sb.Append(",\"clock_dt\":").Append(N(mc.LastDeltaTime));
            // The carried weight as the game sums it (0.5.0): it changes
            // under the player, with every magazine dropped.
            if (player.Inventory != null)
            {
                sb.Append(",\"inventory_weight\":").Append(N(player.Inventory.TotalWeight.Value));
            }
            sb.Append(",\"frame\":").Append(_frame++);
            sb.Append(",\"move\":").Append(V(_free ? Vector2.zero : new Vector2(
                (_current.IsHeld("d", _time) ? 1f : 0f) - (_current.IsHeld("a", _time) ? 1f : 0f),
                (_current.IsHeld("w", _time) ? 1f : 0f) - (_current.IsHeld("s", _time) ? 1f : 0f))));
            sb.Append(",\"pos\":").Append(V(root.position - _origin));
            sb.Append(",\"vel\":").Append(V(mc.Velocity));
            sb.Append(",\"motion\":").Append(V(mc.InputMotion));
            sb.Append(",\"look\":").Append(V(mc.Rotation));
            sb.Append(",\"body_yaw\":").Append(N(root.rotation.eulerAngles.y));
            sb.Append(",\"state\":\"").Append(mc.CurrentState.Name.ToString()).Append('"');
            sb.Append(",\"prone\":").Append(B(mc.IsInPronePose));
            AppendAnimator(sb, "body", mc.PlayerAnimator != null ? mc.PlayerAnimator.Animator : null, 0);
            sb.Append(",\"speed\":").Append(N(mc.CharacterMovementSpeed));
            sb.Append(",\"smoothed_speed\":").Append(N(mc.SmoothedCharacterMovementSpeed));
            sb.Append(",\"clamped_speed\":").Append(N(mc.ClampedSpeed));
            sb.Append(",\"dir\":").Append(V(mc.MovementDirection));
            sb.Append(",\"pose\":").Append(N(mc.PoseLevel));
            sb.Append(",\"smoothed_pose\":").Append(N(mc.SmoothedPoseLevel));
            sb.Append(",\"tilt\":").Append(N(mc.Tilt));
            sb.Append(",\"smoothed_tilt\":").Append(N(mc.SmoothedTilt));
            sb.Append(",\"sprint_speed\":").Append(N(mc.SprintSpeed));
            sb.Append(",\"sprinting\":").Append(B(player.Physical.Sprinting));
            sb.Append(",\"grounded\":").Append(B(mc.IsGrounded));
            // The ground under the player as the game takes it, and how the
            // body is laid to it when lying down.
            sb.Append(",\"surface_normal\":").Append(V(mc.SurfaceNormal));
            sb.Append(",\"surface_pitch\":").Append(N(mc.SurfacePitch));
            if (player.PlayerBones != null && player.PlayerBones.AnimatedTransform != null && player.PlayerBones.AnimatedTransform.Original != null)
                sb.Append(",\"body_tilt\":").Append(Q(player.PlayerBones.AnimatedTransform.Original.localRotation));
            sb.Append(",\"freefall\":").Append(N(mc.FreefallTime));
            sb.Append(",\"stamina\":").Append(N(player.Physical.Stamina.Current));
            sb.Append(",\"hands_stamina\":").Append(N(player.Physical.HandsStamina.Current));
            if (pwa != null && pwa.HandsContainer != null)
            {
                var hc = pwa.HandsContainer;
                sb.Append(",\"aiming\":").Append(B(pwa.IsAiming));
                sb.Append(",\"hands_pos\":").Append(V(hc.HandsPosition.Current));
                sb.Append(",\"hands_pos_zero\":").Append(V(hc.HandsPosition.Zero));
                sb.Append(",\"hands_rot\":").Append(V(hc.HandsRotation.Current));
                sb.Append(",\"hands_rot_zero\":").Append(V(hc.HandsRotation.Zero));
                sb.Append(",\"sway\":").Append(V(hc.SwaySpring.Value));
                sb.Append(",\"cam_rot\":").Append(V(hc.CameraRotation.Current));
                sb.Append(",\"cam_rot_zero\":").Append(V(hc.CameraRotation.Zero));
                sb.Append(",\"cam_pos\":").Append(V(hc.CameraPosition.Current));
                sb.Append(",\"step_frequency\":").Append(N(pwa.Walk.StepFrequency));
                sb.Append(",\"walk_intensity\":").Append(N(pwa.Walk.Intensity));
                sb.Append(",\"possible_tilt\":").Append(N(pwa.PossibleTilt));
                sb.Append(",\"intensity_by_aiming\":").Append(N(pwa.IntensityByAiming));
                sb.Append(",\"mask\":").Append((int)pwa.Mask);
                // How far the weapon would stick into something, and what
                // the game does to it for that.
                var away = pwa.TurnAway;
                if (away != null)
                {
                    sb.Append(",\"overlap_depth\":").Append(N(away.OverlapDepth));
                    sb.Append(",\"turn_away\":").Append(N(away.OverlapValue));
                    sb.Append(",\"turn_away_pos\":").Append(V(away.Position));
                    sb.Append(",\"turn_away_rot\":").Append(V(away.Rotation));
                }
                var recoil = pwa.Shootingg != null ? pwa.Shootingg.NewShotRecoil : null;
                if (recoil != null && recoil.HandRotationRecoil != null)
                {
                    sb.Append(",\"recoil_rot\":").Append(V(recoil.HandRotationRecoil.Current));
                    sb.Append(",\"recoil_pos\":").Append(V(recoil.HandPositionRecoil.Current));
                    sb.Append(",\"recoil_cam\":").Append(V(recoil.CameraRotationRecoil.Current));
                    sb.Append(",\"recoil_stable\":").Append(B(recoil.HandRotationRecoil.StableOn));
                    sb.Append(",\"recoil_shot\":").Append(recoil._autoFireShotIndex);
                }
                sb.Append(",\"camera_smooth\":").Append(N(pwa.CameraSmoothBlender.Value));
                if (hc.WeaponRootAnim != null)
                {
                    sb.Append(",\"weapon_pos\":").Append(V(hc.WeaponRootAnim.localPosition));
                    sb.Append(",\"weapon_rot\":").Append(Q(hc.WeaponRootAnim.localRotation));
                    // How much the weapon is squeezed as drawn, and where its
                    // moving bone is as the camera sees it.
                    sb.Append(",\"weapon_scale\":").Append(V(hc.WeaponRootAnim.lossyScale));
                    if (hc.CameraTransform != null)
                    {
                        sb.Append(",\"weapon_seen\":").Append(V(hc.CameraTransform.InverseTransformPoint(hc.WeaponRootAnim.position)));
                        if (hc.Fireport != null)
                        {
                            sb.Append(",\"muzzle_seen\":").Append(V(hc.CameraTransform.InverseTransformPoint(hc.Fireport.position)));
                        }
                    }
                }
                if (hc.CameraTransform != null)
                {
                    sb.Append(",\"camera_pos\":").Append(V(hc.CameraTransform.localPosition));
                    sb.Append(",\"camera_rot\":").Append(Q(hc.CameraTransform.localRotation));
                    sb.Append(",\"camera_world_rot\":").Append(Q(hc.CameraTransform.rotation));
                    // Where the eye is relative to the feet, in the body's axes.
                    sb.Append(",\"eye\":").Append(V(root.InverseTransformPoint(hc.CameraTransform.position)));
                }
            }
            var firearm = player.HandsController as Player.FirearmController;
            if (firearm != null && firearm.Item != null)
            {
                sb.Append(",\"chamber\":").Append(firearm.Item.ChamberAmmoCount);
                sb.Append(",\"magazine\":").Append(firearm.Item.GetCurrentMagazineCount());
                sb.Append(",\"auto_fire\":").Append(B(firearm.autoFireOn));
                sb.Append(",\"trigger\":").Append(B(firearm.IsTriggerPressed));
                // The mode the weapon is set to (0.6.2).
                sb.Append(",\"fire_mode\":\"").Append(firearm.Item.SelectedFireMode.ToString()).Append('"');
                AppendWeaponParameters(sb, firearm);
                var hands = firearm.FirearmsAnimator != null ? firearm.FirearmsAnimator.Animator : null;
                if (hands != null)
                {
                    AppendAnimator(sb, "hands", hands, Mathf.Max(0, hands.GetLayerIndex("Hands")));
                }
            }
            AppendHandsOnTheWorld(sb, player, firearm == null);
            AppendCarried(sb, player);
            var cam = Camera.main;
            if (cam != null)
            {
                sb.Append(",\"fov\":").Append(N(cam.fieldOfView));
            }
            AppendWider(sb, player);
            AppendGround(sb, player);
            AppendLegs(sb, player);
            AppendShared(sb, player);
            AppendCondition(sb, player);
            AppendVaulting(sb, player);
            sb.Append('}');
            _trace.WriteLine(sb.ToString());
        }

        // ------------------------------------------------- the wider record

        /// Parameters of the body's animator written every frame, by kind.
        /// The first eleven floats are the curves its clips drive: who has
        /// the weapon and each arm.
        private static readonly string[] AnimatorFloats =
        {
            "Weapon_3rd", "First_Person_Curve_Weight", "Hand_Left", "Hand_Right", "Shoulder_Left", "Shoulder_Right",
            "Elbow_Left", "Elbow_Right", "Position_cache", "Leftstance_curve", "Aiming_Layer", "FootStep", "Utility",
            "Aim_angle", "DeltaRotation", "Direct", "Direct_X", "Direct_Y", "Level", "Speed", "SprintSpeed", "Tilt",
            "TransitionMultiplier", "WeaponTypeFloat", "ReloadFloat", "FallingDownFloat", "WeapSizeModifier",
            "ThirdPersonFloat", "InertFloat", "SidestepFloat", "SidebackSpeed", "SprintInertia", "LeftStanceFloat",
            "BlindFireFloat", "WeaponOverlapFloat", "Mouse_look", "RotateSpeed",
            // What a vault or a climb is chosen and played by (0.4.8).
            "VaultingHeight", "VaultingLength", "OverObstacleHeight", "AbsoluteForwardVelocity", "Vaulting_speed",
        };
        private static readonly string[] AnimatorBools =
        {
            "Inert", "IsJumping", "Landing", "Prone", "Sprint", "IsGrounded", "FallingDown", "isAiming", "LeftStance",
            "ProneReload", "IsVaulting", "Stairs", "DoVault", "DoClimb",
            // The hands on the world and the open inventory (0.6.0).
            "IsInteraction", "Interact", "Approached", "Inventory", "InventoryOperation", "Loot", "Pickup",
            // Whose eyes the body is posed for (0.7.0).
            "ThirdPerson",
        };
        // `FirstAction` (0.7.0): the body's own part in a weapon drawn and
        // put away, which only a character seen from outside shows.
        private static readonly string[] AnimatorInts = { "Step", "WeaponType", "InteractionIndex", "FirstAction" };

        private static readonly int LeftHandProgressHash = Animator.StringToHash("LeftHandProgress");
        private static readonly int UseLeftHandHash = Animator.StringToHash("UseLeftHand");
        private static readonly int HandsInventoryHash = Animator.StringToHash("Inventory");
        private static readonly int HandsActiveHash = Animator.StringToHash("Active");

        /// What the hands are doing to the world, and what holds them
        /// (0.6.0): whether the inventory is open, which controller the
        /// hands have, the thing being worked and how it describes its own
        /// working; and of
        /// the hands' animator the three parameters the game's states for
        /// this ask about, the layer of the left hand's own gestures, and,
        /// where the hands hold no firearm, the state of its hands layer,
        /// which a firearm's has written above. Whatever cannot be read is
        /// left out: a recording is never lost for this.
        private void AppendHandsOnTheWorld(StringBuilder sb, Player player, bool writeHandsLayer)
        {
            try
            {
                var mc = player.MovementContext;
                sb.Append(",\"inventory_open\":").Append(B(player.IsInventoryOpened));
                var controller = player.HandsController;
                sb.Append(",\"hands_controller\":\"").Append(controller != null ? controller.GetType().Name : "none").Append('"');

                var thing = mc.InteractionInfo.WorldInteractiveObject;
                if (thing != null)
                {
                    sb.Append(",\"working\":\"").Append(thing.GetType().Name).Append('"');
                    sb.Append(",\"working_state\":\"").Append(thing.DoorState.ToString()).Append('"');
                    sb.Append(",\"working_animation\":").Append(mc.InteractionParameters.AnimationId);
                    sb.Append(",\"working_snap\":").Append(B(mc.InteractionParameters.Snap));
                    sb.Append(",\"working_grip\":").Append(B(mc.InteractionParameters.Grip != null));
                }
                AppendThing(sb, player);
                AppendKeys(sb);
                var hands = controller != null && controller.FirearmsAnimator != null ? controller.FirearmsAnimator.Animator : null;
                if (hands != null)
                {
                    sb.Append(",\"hands_left_progress\":").Append(N(hands.GetFloat(LeftHandProgressHash)));
                    sb.Append(",\"hands_use_left\":").Append(B(hands.GetBool(UseLeftHandHash)));
                    sb.Append(",\"hands_inventory\":").Append(B(hands.GetBool(HandsInventoryHash)));
                    sb.Append(",\"hands_active\":").Append(B(hands.GetBool(HandsActiveHash)));
                    int actions = hands.GetLayerIndex("LActions");
                    if (actions >= 0)
                    {
                        var state = hands.GetCurrentAnimatorStateInfo(actions);
                        sb.Append(",\"hands_actions\":[").Append(state.shortNameHash & 0xFFFFFF).Append(',').Append(N(state.normalizedTime)).Append(',')
                            .Append(N(hands.GetLayerWeight(actions))).Append(']');
                    }
                    if (writeHandsLayer)
                    {
                        AppendAnimator(sb, "hands", hands, Mathf.Max(0, hands.GetLayerIndex("Hands")));
                    }
                }
            }
            catch (Exception)
            {
            }
        }
        /// The door or container last looked at (0.6.1). The fifteenth
        /// recording showed that a door pushed and a container searched
        /// set nothing in the movement context (`working` above is only
        /// there for a thing unlocked or kicked), so the thing is taken
        /// from what the player looks at and kept until another is looked
        /// at: its kind, its own name, its state, the angle it stands at
        /// with the two it rests at, the axis it turns on, where it is
        /// from the player, how it is turned, and what it asks of a
        /// player standing where this one stands (the gesture, whether
        /// the player is put at a place, whether there is a grip).
        private EFT.Interactive.WorldInteractiveObject _thing;
        private void AppendThing(StringBuilder sb, Player player)
        {
            try
            {
                var looked = player.InteractableObject;
                if (looked != null)
                {
                    sb.Append(",\"looking_at\":\"").Append(looked.GetType().Name).Append('"');
                    var seen = looked as EFT.Interactive.WorldInteractiveObject;
                    if (seen != null) _thing = seen;
                }
                if (_thing == null) return;
                var own = new StringBuilder();
                own.Append(",\"thing\":\"").Append(_thing.GetType().Name).Append('"');
                own.Append(",\"thing_id\":\"").Append((_thing.Id ?? "").Replace("\\", "/").Replace("\"", "'")).Append('"');
                own.Append(",\"thing_state\":\"").Append(_thing.DoorState.ToString()).Append('"');
                own.Append(",\"thing_angle\":").Append(N(_thing.CurrentAngle));
                own.Append(",\"thing_angles\":[").Append(N(_thing.CloseAngle)).Append(',').Append(N(_thing.OpenAngle)).Append(']');
                own.Append(",\"thing_axis\":\"").Append(_thing.DoorAxis.ToString()).Append('"');
                own.Append(",\"thing_from_player\":").Append(V(_thing.transform.position - player.Transform.position));
                own.Append(",\"thing_rot\":").Append(Q(_thing.transform.rotation));
                own.Append(",\"thing_without_animation\":").Append(B(_thing.interactWithoutAnimation));
                var asked = _thing.GetInteractionParameters(player.Transform.position);
                own.Append(",\"thing_gesture\":").Append(asked.AnimationId);
                own.Append(",\"thing_snap\":").Append(B(asked.Snap));
                own.Append(",\"thing_grip\":").Append(B(asked.Grip != null));
                sb.Append(own);
            }
            catch (Exception)
            {
            }
        }

        /// The keys that went down in this frame, of those a recording by
        /// hand is made with (0.6.1): the times the hands' work is
        /// measured from. Written only when one did.
        private static readonly KeyCode[] WatchedKeys =
        {
            KeyCode.F, KeyCode.Tab, KeyCode.R, KeyCode.T, KeyCode.B, KeyCode.G, KeyCode.V, KeyCode.Y, KeyCode.U, KeyCode.Q, KeyCode.E,
            KeyCode.C, KeyCode.X, KeyCode.Z, KeyCode.Space, KeyCode.LeftShift, KeyCode.LeftControl, KeyCode.LeftAlt, KeyCode.CapsLock,
            KeyCode.Alpha1, KeyCode.Alpha2, KeyCode.Alpha3, KeyCode.Alpha4, KeyCode.Alpha5, KeyCode.Alpha6, KeyCode.Alpha7, KeyCode.Alpha8,
            KeyCode.Alpha9, KeyCode.Alpha0, KeyCode.Mouse0, KeyCode.Mouse1, KeyCode.Mouse2, KeyCode.Escape, KeyCode.Delete, KeyCode.Return,
        };
        private void AppendKeys(StringBuilder sb)
        {
            try
            {
                StringBuilder down = null, up = null;
                foreach (var key in WatchedKeys)
                {
                    if (Input.GetKeyDown(key))
                    {
                        if (down == null) down = new StringBuilder(); else down.Append(',');
                        down.Append('"').Append(key.ToString()).Append('"');
                    }
                    // The keys as they come up (0.6.2): a click acts when
                    // the button is let go, and so do the quick-use keys.
                    if (Input.GetKeyUp(key))
                    {
                        if (up == null) up = new StringBuilder(); else up.Append(',');
                        up.Append('"').Append(key.ToString()).Append('"');
                    }
                }
                if (down != null) sb.Append(",\"keys_down\":[").Append(down).Append(']');
                if (up != null) sb.Append(",\"keys_up\":[").Append(up).Append(']');
                // With either, which of the keys a click goes by are held,
                // and where the mouse is on the screen.
                if (down != null || up != null)
                {
                    sb.Append(",\"held\":[");
                    var first = true;
                    foreach (var key in new[] { KeyCode.LeftControl, KeyCode.RightControl, KeyCode.LeftAlt, KeyCode.RightAlt, KeyCode.LeftShift, KeyCode.RightShift })
                    {
                        if (!Input.GetKey(key)) continue;
                        if (!first) sb.Append(',');
                        first = false;
                        sb.Append('"').Append(key.ToString()).Append('"');
                    }
                    sb.Append(']');
                    sb.Append(",\"mouse\":").Append(V(new Vector2(Input.mousePosition.x, Input.mousePosition.y)));
                    sb.Append(",\"screen\":").Append(V(new Vector2(Screen.width, Screen.height)));
                }
            }
            catch (Exception)
            {
            }
        }

        /// What the character carries and what the container last looked
        /// at holds, whenever either has changed (0.6.2): the moves made
        /// with the mouse and the keys in the inventory screen leave no
        /// other trace. Each list is as the header's: every item with
        /// what it is in, where, its stack, its cells and its weight. With
        /// them what is bound to the quick-use keys and which item is in
        /// the hands, each when it changes.
        private int _carriedSum, _lootSum;
        private string _boundWas = "", _inHandsWas = "";
        private bool _carriedFailed;

        private static int ListSum(IEnumerable<Item> items)
        {
            unchecked
            {
                int sum = 17;
                foreach (var item in items)
                {
                    sum = sum * 31 + item.Id.GetHashCode();
                    sum = sum * 31 + item.StackObjectsCount;
                    var address = item.CurrentAddress;
                    if (address == null || address.Container == null) continue;
                    sum = sum * 31 + address.Container.ID.GetHashCode();
                    if (address.Container.ParentItem != null) sum = sum * 31 + address.Container.ParentItem.Id.GetHashCode();
                    var cell = address as GridItemAddress;
                    if (cell != null) sum = sum * 31 + cell.LocationInGrid.x * 4096 + cell.LocationInGrid.y * 2 + (cell.LocationInGrid.r == ItemRotation.Vertical ? 1 : 0);
                }
                return sum;
            }
        }

        private static void AppendItems(StringBuilder sb, string name, Item root)
        {
            sb.Append(",\"").Append(name).Append("\":[");
            var first = true;
            foreach (var item in root.GetAllItems())
            {
                if (!first) sb.Append(',');
                first = false;
                sb.Append("{\"id\":\"").Append(item.Id).Append('"');
                sb.Append(",\"tpl\":\"").Append(item.TemplateId.ToString()).Append('"');
                var address = item.CurrentAddress;
                if (address != null && address.Container != null && item != root)
                {
                    var holder = address.Container.ParentItem;
                    if (holder != null) sb.Append(",\"in\":\"").Append(holder.Id).Append('"');
                    sb.Append(",\"at\":\"").Append(address.Container.ID).Append('"');
                    var cell = address as GridItemAddress;
                    if (cell != null)
                    {
                        sb.Append(",\"x\":").Append(cell.LocationInGrid.x);
                        sb.Append(",\"y\":").Append(cell.LocationInGrid.y);
                        sb.Append(",\"r\":").Append(cell.LocationInGrid.r == ItemRotation.Vertical ? 1 : 0);
                    }
                }
                sb.Append(",\"n\":").Append(item.StackObjectsCount);
                var size = item.CalculateCellSize();
                sb.Append(",\"w\":").Append(size.X).Append(",\"h\":").Append(size.Y);
                sb.Append(",\"kg\":").Append(N(item.TotalWeight));
                sb.Append('}');
            }
            sb.Append(']');
        }

        private void AppendCarried(StringBuilder sb, Player player)
        {
            if (_carriedFailed) return;
            var own = new StringBuilder();
            try
            {
                var inventory = player.Inventory;
                if (inventory == null || inventory.Equipment == null) return;
                var sum = ListSum(inventory.Equipment.GetAllItems());
                if (sum != _carriedSum)
                {
                    _carriedSum = sum;
                    AppendItems(own, "equipment", inventory.Equipment);
                }
                var container = _thing as EFT.Interactive.LootableContainer;
                var loot = container != null && container.ItemOwner != null ? container.ItemOwner.RootItem : null;
                if (loot != null)
                {
                    var lootSum = ListSum(loot.GetAllItems());
                    if (lootSum != _lootSum)
                    {
                        _lootSum = lootSum;
                        AppendItems(own, "loot", loot);
                    }
                }
                var bound = new StringBuilder();
                foreach (var pair in inventory.FastAccess.BoundItems)
                {
                    if (pair.Value == null) continue;
                    if (bound.Length > 0) bound.Append(',');
                    bound.Append('"').Append(pair.Key.ToString()).Append("\":\"").Append(pair.Value.Id).Append('"');
                }
                if (bound.ToString() != _boundWas)
                {
                    _boundWas = bound.ToString();
                    own.Append(",\"bound\":{").Append(_boundWas).Append('}');
                }
                var held = player.HandsController != null && player.HandsController.Item != null ? player.HandsController.Item.Id.ToString() : "";
                if (held != _inHandsWas)
                {
                    _inHandsWas = held;
                    own.Append(",\"in_hands\":\"").Append(held).Append('"');
                }
                sb.Append(own);
            }
            catch (Exception e)
            {
                _carriedFailed = true;
                Logger.LogWarning("what is carried is left out from here on: " + e);
            }
        }

        /// The parameters of the animator of the weapon in hand, each when
        /// it changes (0.6.2), as `hp_<name>`: what the weapon's logic
        /// tells its animator (the rounds in the magazine and the chamber
        /// as it has them, the fire mode, the flags its states go by).
        /// They are the engine's own list for that animator, found once a
        /// weapon; triggers are left out.
        private object _parametersOf;
        private Animator _weaponAnimator;
        private AnimatorControllerParameter[] _weaponParameters;
        private float[] _weaponValues;

        private void AppendWeaponParameters(StringBuilder sb, Player.FirearmController firearm)
        {
            try
            {
                var wrapper = firearm.FirearmsAnimator != null ? (object)firearm.FirearmsAnimator.Animator : null;
                if (wrapper == null) return;
                if (!ReferenceEquals(wrapper, _parametersOf))
                {
                    _parametersOf = wrapper;
                    _weaponAnimator = null;
                    foreach (var field in wrapper.GetType().GetFields(AnyInstance))
                    {
                        if (field.FieldType != typeof(Animator)) continue;
                        _weaponAnimator = field.GetValue(wrapper) as Animator;
                        break;
                    }
                    _weaponParameters = _weaponAnimator != null ? _weaponAnimator.parameters : null;
                    _weaponValues = _weaponParameters != null ? new float[_weaponParameters.Length] : null;
                    if (_weaponValues != null) for (int i = 0; i < _weaponValues.Length; i++) _weaponValues[i] = float.NaN;
                }
                if (_weaponAnimator == null || _weaponParameters == null) return;
                for (int i = 0; i < _weaponParameters.Length; i++)
                {
                    var parameter = _weaponParameters[i];
                    float value;
                    switch (parameter.type)
                    {
                        case AnimatorControllerParameterType.Float: value = _weaponAnimator.GetFloat(parameter.nameHash); break;
                        case AnimatorControllerParameterType.Int: value = _weaponAnimator.GetInteger(parameter.nameHash); break;
                        case AnimatorControllerParameterType.Bool: value = _weaponAnimator.GetBool(parameter.nameHash) ? 1f : 0f; break;
                        default: continue;
                    }
                    if (value == _weaponValues[i]) continue;
                    _weaponValues[i] = value;
                    sb.Append(",\"hp_").Append(parameter.name).Append("\":").Append(N(value));
                }
            }
            catch (Exception)
            {
                _weaponAnimator = null;
            }
        }
        private static readonly int[] FloatHashes = Array.ConvertAll(AnimatorFloats, Animator.StringToHash);
        private static readonly int[] BoolHashes = Array.ConvertAll(AnimatorBools, Animator.StringToHash);
        private static readonly int[] IntHashes = Array.ConvertAll(AnimatorInts, Animator.StringToHash);

        private const BindingFlags AnyInstance = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;
        private static readonly FieldInfo TacticalReloadField = typeof(ProceduralWeaponAnimation).GetField("_tacticalReload", AnyInstance);
        private static readonly FieldInfo RightHandField = typeof(Player).GetField("_firstPersonRightHand", AnyInstance);
        private static readonly FieldInfo LeftHandField = typeof(Player).GetField("_firstPersonLeftHand", AnyInstance);
        private static readonly FieldInfo ElbowBendsField = typeof(Player).GetField("_elbowBends", AnyInstance);
        private static readonly PropertyInfo AuthorityProperty = typeof(Player).GetProperty("ThirdPersonWeaponRootAuthority", AnyInstance);

        /// Set when the wider record threw once; it is then left out so that
        /// the rest of the trace is still written.
        private bool _widerFailed;
        /// The weapon's two hand markers, looked up once per weapon.
        private Transform _markersOf, _markerRight, _markerLeft;

        private static Transform FindDeep(Transform under, string name)
        {
            if (under == null) return null;
            if (under.name == name) return under;
            for (int i = 0; i < under.childCount; i++)
            {
                var found = FindDeep(under.GetChild(i), name);
                if (found != null) return found;
            }
            return null;
        }

        /// A bone's place and turning in another's axes.
        private static void AppendBone(StringBuilder sb, string name, Transform frame, Transform bone)
        {
            if (frame == null || bone == null) return;
            sb.Append(",\"").Append(name).Append("_p\":").Append(V(frame.InverseTransformPoint(bone.position)));
            sb.Append(",\"").Append(name).Append("_q\":").Append(Q(Quaternion.Inverse(frame.rotation) * bone.rotation));
        }

        /// What the comparison of aiming, of the hands' hold and of the
        /// body's arms needs, beyond the rest of the frame.
        private void AppendWider(StringBuilder sb, Player player)
        {
            if (_widerFailed) return;
            var wider = new StringBuilder(2048);
            try
            {
                var mc = player.MovementContext;
                var pwa = player.ProceduralWeaponAnimation;
                var root = player.Transform.Original;
                var firearm = player.HandsController as Player.FirearmController;

                // Aiming: whether the weapon's logic has it, whether the
                // movement state allows it, and how far a reload with the
                // sights up has stiffened the weapon.
                wider.Append(",\"can_aim\":").Append(B(EFTHardSettings.Instance.CanAimInState(mc.CurrentState.Name)));
                if (firearm != null)
                {
                    wider.Append(",\"firearm_aiming\":").Append(B(firearm.IsAiming));
                }
                if (pwa != null)
                {
                    wider.Append(",\"tactical_reload_on\":").Append(B(pwa.TacticalReload));
                    if (TacticalReloadField != null && TacticalReloadField.GetValue(pwa) is Player.ValueBlender blender)
                    {
                        wider.Append(",\"tactical_reload\":").Append(N(blender.Value));
                    }
                }
                if (RightHandField != null) wider.Append(",\"right_hand_first_person\":").Append(N((float)RightHandField.GetValue(player)));
                if (LeftHandField != null) wider.Append(",\"left_hand_first_person\":").Append(N((float)LeftHandField.GetValue(player)));
                if (AuthorityProperty != null) wider.Append(",\"weapon_3rd_authority\":").Append(N((float)AuthorityProperty.GetValue(player, null)));
                // Whose eyes the player is seen with on this frame (0.7.0).
                wider.Append(",\"third_person\":").Append(B(player.PointOfView == EPointOfView.ThirdPerson));
                // The head's free look (0.7.0): its turn as the camera
                // takes it, whether the mouse has the head, and the
                // engine's two mouse axes as this frame read them, which
                // the turn and the look are both made from.
                wider.Append(",\"head_rot\":").Append(V(player.HeadRotation));
                wider.Append(",\"mouse_look\":").Append(B(player.MouseLookControl));
                wider.Append(",\"head_looking\":").Append(B(player.IsLooking));
                wider.Append(",\"mouse_axes\":[").Append(N(Input.GetAxisRaw("Mouse X"))).Append(',').Append(N(Input.GetAxisRaw("Mouse Y"))).Append(']');
                wider.Append(",\"ground_check\":").Append(N(mc.CheckGroundedRayDistance));
                wider.Append(",\"state_speed_limit\":").Append(N(mc.StateSpeedLimit));

                // The view's turning: what the script asked this frame, what
                // the game multiplies it by, and the movement state's own
                // factor and limit on it (0.4.0).
                wider.Append(",\"turn_asked\":").Append(V(_askedTurn));
                wider.Append(",\"rotation_multiplier\":").Append(N(player.GetRotationMultiplier()));
                wider.Append(",\"turn_factor\":").Append(N(mc.CurrentState.StateSensitivity));
                wider.Append(",\"turn_limit\":").Append(N(mc.CurrentState.RotationSpeedClamp));
                wider.Append(",\"sprint_sensitivity\":").Append(N(player.Physical.SprintSensitivity));
                // Which of its operations the weapon's logic has in hand.
                if (firearm != null && firearm.CurrentOperation != null)
                {
                    wider.Append(",\"operation\":\"").Append(firearm.CurrentOperation.GetType().Name).Append('"');
                }

                // The body's animator: its parameters and every layer.
                var animator = mc.PlayerAnimator != null ? mc.PlayerAnimator.Animator : null;
                if (animator != null)
                {
                    for (int i = 0; i < FloatHashes.Length; i++)
                        wider.Append(",\"p_").Append(AnimatorFloats[i]).Append("\":").Append(N(animator.GetFloat(FloatHashes[i])));
                    for (int i = 0; i < BoolHashes.Length; i++)
                        wider.Append(",\"p_").Append(AnimatorBools[i]).Append("\":").Append(B(animator.GetBool(BoolHashes[i])));
                    for (int i = 0; i < IntHashes.Length; i++)
                        wider.Append(",\"p_").Append(AnimatorInts[i]).Append("\":").Append(animator.GetInteger(IntHashes[i]));
                    // Four numbers a layer: its state, how far through it,
                    // its weight, and the state it is blending into (-1 for none).
                    wider.Append(",\"layers\":[");
                    int layers = animator.layerCount;
                    for (int layer = 0; layer < layers; layer++)
                    {
                        var current = animator.GetCurrentAnimatorStateInfo(layer);
                        if (layer > 0) wider.Append(',');
                        wider.Append(current.shortNameHash & 0xFFFFFF).Append(',').Append(N(current.normalizedTime)).Append(',');
                        wider.Append(N(animator.GetLayerWeight(layer))).Append(',');
                        wider.Append(animator.IsInTransition(layer) ? (animator.GetNextAnimatorStateInfo(layer).shortNameHash & 0xFFFFFF) : -1);
                    }
                    wider.Append(']');
                }

                // The body's bones the first-person view takes from, in the
                // player's axes.
                var bones = player.PlayerBones;
                if (bones != null)
                {
                    AppendBone(wider, "ribcage", root, bones.Ribcage != null ? bones.Ribcage.Original : null);
                    // The neck and the head (0.7.0): drawn for other eyes
                    // only, and turned after the animator by the game's code.
                    AppendBone(wider, "neck", root, bones.Neck);
                    AppendBone(wider, "head", root, bones.Head != null ? bones.Head.Original : null);
                    AppendBone(wider, "right_palm", root, bones.RightPalm);
                    AppendBone(wider, "left_palm", root, bones.LeftPalm);
                    AppendBone(wider, "weapon_3rd", root, bones.Weapon_Root_Third);
                    if (bones.Upperarms != null && bones.Upperarms.Length > 1)
                    {
                        AppendBone(wider, "left_upperarm", root, bones.Upperarms[0]);
                        AppendBone(wider, "right_upperarm", root, bones.Upperarms[1]);
                    }
                    if (bones.Forearms != null && bones.Forearms.Length > 1)
                    {
                        AppendBone(wider, "left_forearm", root, bones.Forearms[0]);
                        AppendBone(wider, "right_forearm", root, bones.Forearms[1]);
                    }
                    // The points the arms' solver bends the elbows toward, as
                    // the frame's elbow adjustment left them (0.4.0), and the
                    // weapon's own two, which that adjustment draws on.
                    if (bones.BendGoals != null && bones.BendGoals.Length > 1)
                    {
                        AppendBone(wider, "left_bend_goal", root, bones.BendGoals[0]);
                        AppendBone(wider, "right_bend_goal", root, bones.BendGoals[1]);
                    }
                    if (ElbowBendsField != null && ElbowBendsField.GetValue(player) is Transform[] weaponGoals && weaponGoals.Length > 1)
                    {
                        AppendBone(wider, "weapon_left_bend_goal", root, weaponGoals[0]);
                        AppendBone(wider, "weapon_right_bend_goal", root, weaponGoals[1]);
                    }
                }

                // The weapon's own bone, which its clips move and a reload
                // with the sights up holds still, and its two hand markers,
                // in the axes of the bone the procedural motion moves.
                var hc = pwa != null ? pwa.HandsContainer : null;
                if (hc != null && hc.WeaponRootAnim != null)
                {
                    if (hc.Weapon != null)
                    {
                        wider.Append(",\"weapon_bone_p\":").Append(V(hc.Weapon.localPosition));
                        wider.Append(",\"weapon_bone_q\":").Append(Q(hc.Weapon.localRotation));
                    }
                    if (_markersOf != hc.WeaponRootAnim)
                    {
                        _markersOf = hc.WeaponRootAnim;
                        _markerRight = FindDeep(hc.WeaponRootAnim, "weapon_R_IK_marker");
                        _markerLeft = FindDeep(hc.WeaponRootAnim, "weapon_L_IK_marker");
                    }
                    AppendBone(wider, "right_marker", hc.WeaponRootAnim, _markerRight);
                    AppendBone(wider, "left_marker", hc.WeaponRootAnim, _markerLeft);
                    // Where the rig hangs, in the player's axes.
                    AppendBone(wider, "weapon_root_anim", root, hc.WeaponRootAnim);
                }
            }
            catch (Exception e)
            {
                _widerFailed = true;
                Logger.LogWarning("the wider record is left out from here on: " + e);
                return;
            }
            sb.Append(wider);
        }

        private bool _groundFailed;

        private bool _sharedFailed;

        /// Two figures that change under the player during a raid (0.4.5).
        /// The curve the walk's lowest speed comes from is one object for
        /// every character in the raid, and each of them, a bot too, rewrites
        /// its second key with its own inertia whenever its carried weight
        /// changes: `floor_key` is that key now, time then value. And the
        /// length of the line cast along the weapon is measured afresh when
        /// a reload ends: `weapon_length_now` is the figure in force.
        private void AppendShared(StringBuilder sb, Player player)
        {
            if (_sharedFailed) return;
            try
            {
                var keys = EFTHardSettings.Instance.MovementAccelerationRange.keys;
                if (keys.Length > 1)
                {
                    sb.Append(",\"floor_key\":[").Append(N(keys[1].time)).Append(',').Append(N(keys[1].value)).Append(']');
                }
                var firearm = player.HandsController as Player.FirearmController;
                if (firearm != null)
                {
                    sb.Append(",\"weapon_length_now\":").Append(N(firearm.WeaponLn));
                }
            }
            catch (Exception e)
            {
                _sharedFailed = true;
                Logger.LogWarning("the walk's floor key and the weapon's length are left out from here on: " + e);
            }
        }

        private bool _vaultingFailed;

        /// What vaulting goes by (0.4.8): the obstacle as the game's own
        /// component last measured it (its height, its length, the drop
        /// behind it, the forward speed the grid was put ahead by), which is
        /// there whether or not a move was chosen; whether the vault key
        /// counts as held; and the character's capsule, which a vault and a
        /// climb shrink from below (its height, how far over the feet its
        /// centre is, its radius).
        private void AppendVaulting(StringBuilder sb, Player player)
        {
            if (_vaultingFailed) return;
            try
            {
                var measured = player.VaultingParameters;
                if (measured != null)
                {
                    sb.Append(",\"vault_obstacle\":[").Append(N(measured.VaultingHeight)).Append(',')
                        .Append(N(measured.VaultingLength)).Append(',').Append(N(measured.BehindObstacleRatio)).Append(',')
                        .Append(N(measured.AbsoluteForwardVelocity)).Append(']');
                }
                sb.Append(",\"vault_key\":").Append(B(player.IsVaultingPressed));
                var capsule = player.MovementContext.CharacterController;
                sb.Append(",\"capsule\":[").Append(N(capsule.height)).Append(',').Append(N(capsule.center.y)).Append(',')
                    .Append(N(capsule.radius)).Append(']');
            }
            catch (Exception e)
            {
                _vaultingFailed = true;
                Logger.LogWarning("the vaulting figures are left out from here on: " + e);
            }
        }

        private bool _conditionFailed;

        /// What stamina goes by besides the two pools written since the
        /// first version (0.4.7): how well fed and watered the character is,
        /// both of which fall all through a raid and scale every refill and
        /// every drain, the oxygen, the three capacities, and the two skills
        /// the capacities, the refill and the costs go by. Only values the
        /// game itself reads every frame are read, so that reading them here
        /// changes nothing in the game.
        private void AppendCondition(StringBuilder sb, Player player)
        {
            if (_conditionFailed) return;
            try
            {
                var physical = player.Physical;
                var health = player.HealthController;
                sb.Append(",\"energy\":").Append(N(health.Energy.Normalized));
                sb.Append(",\"hydration\":").Append(N(health.Hydration.Normalized));
                sb.Append(",\"oxygen\":").Append(N(physical.Oxygen.Current));
                sb.Append(",\"stamina_capacity\":").Append(N(physical.Stamina.TotalCapacity.Value));
                sb.Append(",\"hands_capacity\":").Append(N(physical.HandsStamina.TotalCapacity.Value));
                sb.Append(",\"oxygen_capacity\":").Append(N(physical.Oxygen.TotalCapacity.Value));
                sb.Append(",\"endurance\":").Append(player.Skills.Endurance.Level);
                sb.Append(",\"strength\":").Append(player.Skills.Strength.Level);
            }
            catch (Exception e)
            {
                _conditionFailed = true;
                Logger.LogWarning("energy, hydration, oxygen and the skills are left out from here on: " + e);
            }
        }

        private bool _legsFailed;
        private Transform _legsOf;
        private Transform[] _legs;
        private static readonly string[] LegBones =
        {
            "Base HumanPelvis", "Base HumanSpine1",
            "Base HumanLThigh1", "Base HumanLThigh2", "Base HumanLCalf", "Base HumanLFoot", "Base HumanLToe",
            "Base HumanRThigh1", "Base HumanRThigh2", "Base HumanRCalf", "Base HumanRFoot", "Base HumanRToe",
        };
        private static readonly string[] LegNames =
        {
            "pelvis", "spine1",
            "left_thigh1", "left_thigh2", "left_calf", "left_foot", "left_toe",
            "right_thigh1", "right_thigh2", "right_calf", "right_foot", "right_toe",
        };

        /// The lower body's bones as the frame is drawn (0.4.4), in the
        /// player's axes: the pelvis, the first spine bone and each leg down
        /// to the toe. In first person the body is drawn from the hips down,
        /// and these are what its legs are doing, the feet's own solver
        /// included.
        private void AppendLegs(StringBuilder sb, Player player)
        {
            if (_legsFailed) return;
            try
            {
                var bones = player.PlayerBones;
                if (bones == null || bones.RootJoint == null) return;
                if (_legsOf != bones.RootJoint)
                {
                    _legsOf = bones.RootJoint;
                    _legs = Array.ConvertAll(LegBones, name => FindDeep(bones.RootJoint, name));
                }
                var root = player.Transform.Original;
                var legs = new StringBuilder(1024);
                for (int i = 0; i < _legs.Length; i++)
                {
                    AppendBone(legs, LegNames[i], root, _legs[i]);
                }
                sb.Append(legs);
            }
            catch (Exception e)
            {
                _legsFailed = true;
                Logger.LogWarning("the legs' bones are left out from here on: " + e);
            }
        }

        /// How each part of the body is drawn for this player (0.4.4): for
        /// each of its skins, whether its renderers are on and how they
        /// cast shadows. In first person the game draws the legs and the
        /// hands and leaves the rest to cast shadows only.
        private void AppendBodyParts(StringBuilder sb, Player player)
        {
            try
            {
                var body = player.PlayerBody;
                if (body == null) return;
                var parts = new StringBuilder(256);
                parts.Append(",\"point_of_view\":\"").Append(body.PointOfView.Value.ToString()).Append('"');
                foreach (var skin in body.BodySkins)
                {
                    if (skin.Value == null) continue;
                    string drawn = "none";
                    foreach (var renderer in skin.Value.GetRenderers())
                    {
                        if (renderer == null) continue;
                        drawn = (renderer.enabled ? "on" : "off") + " " + renderer.shadowCastingMode;
                        break;
                    }
                    parts.Append(",\"skin_").Append(skin.Key.ToString()).Append("\":\"").Append(drawn).Append('"');
                }
                sb.Append(parts);
            }
            catch (Exception e)
            {
                Logger.LogWarning("how the body's parts are drawn is left out: " + e);
            }
        }

        /// How far under the character's feet the world is (0.4.3). The
        /// character counts as standing within its ground check and rides
        /// that far over the ground; what the game casts at the world (the
        /// weapon's line, the camera's box) meets the ground itself. The
        /// ray is cast on the layers the weapon's line is, less the
        /// player's own.
        private void AppendGround(StringBuilder sb, Player player)
        {
            if (_groundFailed) return;
            try
            {
                const float Above = 0.5f;
                int layers = EFTHardSettings.Instance.WEAPON_OCCLUSION_LAYERS & ~(1 << LayerMask.NameToLayer("Player"));
                var from = player.Transform.Original.position + Vector3.up * Above;
                if (Physics.Raycast(from, Vector3.down, out var hit, Above + 2f, layers, QueryTriggerInteraction.Ignore))
                {
                    sb.Append(",\"ground_below\":").Append(N(hit.distance - Above));
                }
            }
            catch (Exception e)
            {
                _groundFailed = true;
                Logger.LogWarning("the ground under the feet is left out from here on: " + e);
            }
        }
    }

    /// Replaces the player's movement and look input while a scenario runs.
    [HarmonyPatch(typeof(MoveInputTranslator), nameof(MoveInputTranslator.TranslateAxes))]
    internal static class AxesPatch
    {
        private static void Prefix(ref float[] axes)
        {
            var plugin = Plugin.Instance;
            if (plugin != null && plugin.Running && axes != null && axes.Length >= 4)
            {
                plugin.FillAxes(axes);
            }
        }
    }
}
