using System.Collections.Generic;
using System.IO;
using Newtonsoft.Json;

namespace Stlkreft.Oracle
{
    /// <summary>
    /// A scripted input sequence. The same files are read by the Rust
    /// replay tool, so the two sides see identical input.
    /// </summary>
    public class Scenario
    {
        public class Hold
        {
            /// w a s d shift q e aim fire
            [JsonProperty("key")] public string Key;
            [JsonProperty("from")] public float From;
            [JsonProperty("to")] public float To;
        }

        public class Turn
        {
            [JsonProperty("from")] public float From;
            [JsonProperty("to")] public float To;
            /// Degrees per second; yaw to the right, pitch downward.
            [JsonProperty("yaw")] public float Yaw;
            [JsonProperty("pitch")] public float Pitch;
        }

        public class Press
        {
            /// jump crouch prone reload wheel_up wheel_down stance_up stance_down auto single
            [JsonProperty("key")] public string Key;
            [JsonProperty("at")] public float At;
        }

        /// A place where the script's clock stands still until the
        /// character's legs are where the rest of the script needs them.
        /// Every other time in a script is that clock's.
        public class Gate
        {
            /// The script's time the clock stands at while the gate is shut.
            [JsonProperty("at")] public float At;
            /// It opens when the legs hold less than this,
            [JsonProperty("legs_below")] public float? LegsBelow;
            /// or, with no such figure, when they hold this or more.
            [JsonProperty("legs_from")] public float LegsFrom;
            /// A key pressed again and again while it is shut, as a press
            /// of the script's is, and how many seconds apart.
            [JsonProperty("press")] public string Press;
            [JsonProperty("every")] public float Every;
            /// The longest it stays shut, in seconds.
            [JsonProperty("timeout")] public float Timeout;
        }

        [JsonProperty("name")] public string Name;
        /// How long the script runs, by its own clock: the time its gates
        /// are shut comes on top.
        [JsonProperty("duration")] public float Duration;
        [JsonProperty("hold")] public List<Hold> Holds = new List<Hold>();
        [JsonProperty("turn")] public List<Turn> Turns = new List<Turn>();
        [JsonProperty("press")] public List<Press> Presses = new List<Press>();
        [JsonProperty("gate")] public List<Gate> Gates = new List<Gate>();

        public static Scenario Load(string path)
        {
            var scenario = JsonConvert.DeserializeObject<Scenario>(File.ReadAllText(path));
            if (string.IsNullOrEmpty(scenario.Name))
            {
                scenario.Name = Path.GetFileNameWithoutExtension(path);
            }
            scenario.Gates.Sort((a, b) => a.At.CompareTo(b.At));
            return scenario;
        }

        public bool IsHeld(string key, float time)
        {
            foreach (var hold in Holds)
            {
                if (hold.Key == key && time >= hold.From && time < hold.To)
                {
                    return true;
                }
            }
            return false;
        }

        public void TurnRate(float time, out float yaw, out float pitch)
        {
            yaw = 0f;
            pitch = 0f;
            foreach (var turn in Turns)
            {
                if (time >= turn.From && time < turn.To)
                {
                    yaw += turn.Yaw;
                    pitch += turn.Pitch;
                }
            }
        }
    }
}
