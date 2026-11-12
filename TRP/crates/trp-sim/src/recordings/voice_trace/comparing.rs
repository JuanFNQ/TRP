//! Comparing what was said with the recording, trace by trace.

use super::*;

/// What one comparison came to, for the summary.
#[derive(Debug, Default, Clone)]
pub struct Tally {
    pub recorded: usize,
    pub same_frame: usize,
    pub clip_drawable: usize,
    pub level: usize,
    pub reach: usize,
    pub source: usize,
    pub placed: usize,
    pub group: usize,
    pub ours_unrecorded: usize,
    pub ours_out_of_reach: usize,
    /// Ours with none recorded where the recording cannot tell (a
    /// bleeding's count, a blast's last part).
    pub ours_unknown: usize,
    pub painkillers: usize,
    pub ai: usize,
    pub exhaustion: usize,
    pub lines: Vec<String>,
}

/// The phrases the rule said for each bot set beside the recorded ones:
/// `voices` maps a voice id to its name and banks (tags and clips' names).
pub fn compare_all(setups: &[Setup], frames: &BTreeMap<i64, Frame>, hits: &BTreeMap<i64, Vec<BodyHit>>, spoken: &[Spoken], voices: &HashMap<String, (String, Vec<PhraseBank>)>, base_volume: f32) -> Tally {
    let mut tally = Tally::default();
    // Each recorded sound given to the bot whose voice holds its clip and
    // whose head is nearest it then.
    let owner = |s: &Spoken| -> Option<(i64, usize, usize, f32)> {
        let frame = frames.get(&s.frame)?;
        setups
            .iter()
            .filter_map(|setup| {
                let (_, banks) = voices.get(&setup.voice)?;
                let (bank, clip) = banks.iter().enumerate().find_map(|(b, bank)| bank.clips.iter().position(|c| c.name == s.clip).map(|c| (b, c)))?;
                let bot = frame.bots.get(&setup.id)?;
                Some((setup.id, bank, clip, gap(bot.head.unwrap_or(bot.root), s.pos)))
            })
            .min_by(|a, b| a.3.total_cmp(&b.3))
    };
    let mut owned: HashMap<i64, Vec<(&Spoken, usize, usize, f32)>> = HashMap::new();
    for s in spoken {
        match owner(s) {
            Some((id, bank, clip, off)) => owned.entry(id).or_default().push((s, bank, clip, off)),
            None => tally.lines.push(format!("  frame {}: {} of no recorded bot's voice", s.frame, s.clip)),
        }
    }
    for (n, setup) in setups.iter().enumerate() {
        let Some((name, banks)) = voices.get(&setup.voice) else {
            tally.lines.push(format!("bot {} ({}): its voice {} was not read", setup.id, setup.side, setup.voice));
            continue;
        };
        let chained: Vec<i32> = banks.iter().filter(|b| HEALTH.contains(&b.trigger) && b.chain.who == 0).map(|b| b.chain.event).collect();
        let compared = |trigger: i32| HEALTH.contains(&trigger) || chained.contains(&trigger);
        let recorded = owned.remove(&setup.id).unwrap_or_default();
        let heard: Vec<Heard> = recorded.iter().map(|(s, bank, clip, _)| (s.frame, banks[*bank].trigger, *clip)).collect();
        // Muffled all along where its speech was heard muffled; on
        // painkillers or not, whichever the recording bears out better
        // (the phrases said on their frame, less those said with none).
        let muffled = recorded.iter().any(|(s, ..)| s.group == "SimpleOccluded");
        let within = |p: &Phrase| {
            let frame = frames.get(&p.frame);
            let distance = frame.and_then(|f| Some(gap(f.camera?, f.bots.get(&setup.id).and_then(|b| b.head)?)));
            let falloff = banks[p.bank].clips.get(p.clip).map_or(0.0, |c| c.falloff);
            !distance.is_some_and(|d| d > falloff + 1.0)
        };
        let score = |ours: &[(Phrase, Cause)]| -> i64 {
            let same = heard.iter().filter(|&&(f, tr, _)| compared(tr) && ours.iter().any(|(p, _)| p.frame == f && p.trigger == tr)).count() as i64;
            let extra = ours.iter().filter(|(p, _)| within(p) && !heard.iter().any(|&(f, tr, _)| (f - p.frame).abs() <= 3 && tr == p.trigger)).count() as i64;
            2 * same - extra
        };
        let seed = 0x5eed_0000 + n as u64;
        let plain = run(setup, setups, banks, frames, hits, &heard, Given { muffled, painkillers: false }, seed);
        let numb = run(setup, setups, banks, frames, hits, &heard, Given { muffled, painkillers: true }, seed);
        let scores = (score(&plain.0), score(&numb.0));
        let painkillers = scores.1 > scores.0;
        let (ours, notes) = if painkillers { numb } else { plain };
        let hit_count = hits.values().flatten().filter(|h| h.profile == setup.profile).count();
        if hit_count == 0 && recorded.is_empty() && ours.is_empty() {
            continue;
        }
        tally.lines.push(format!(
            "bot {} ({}, voice {name}): {hit_count} hits recorded, {} of its speech recorded, {} phrases of the rule{}{}",
            setup.id,
            setup.side,
            recorded.len(),
            ours.len(),
            if muffled { "; muffled (its speech heard in SimpleOccluded)" } else { "" },
            if painkillers { "; on painkillers all along (the recording bears it out better: a bot's own medicine, the AI's)" } else { "" }
        ));
        if scores.0 != scores.1 {
            tally.lines.push(format!("    (twice the phrases said on their frame less those said with none: {} without a painkiller, {} with)", scores.0, scores.1));
        }
        if painkillers {
            tally.painkillers += 1;
        }
        for note in notes {
            tally.lines.push(format!("    {note}"));
        }
        let mut matched = vec![false; ours.len()];
        for (s, bank, clip, off) in &recorded {
            let b = &banks[*bank];
            let what = format!("frame {}: {} (phrase {}, clip {clip})", s.frame, s.clip, b.trigger);
            if !compared(b.trigger) {
                tally.ai += 1;
                if let Some(i) = ours.iter().position(|(p, _)| p.frame == s.frame && p.trigger == b.trigger) {
                    matched[i] = true;
                }
                tally.lines.push(format!("    {what}: its AI's, not built (told to the speaker as recorded, for how long it keeps it busy)"));
                continue;
            }
            let tags_of = |c: usize| b.clips.get(c).map_or(0, |c| c.mask);
            if b.trigger == trigger::BREATH && compare(tags_of(*clip), tag::AWARE) && !compare(tags_of(*clip), tag::UNAWARE) {
                tally.exhaustion += 1;
                if let Some(i) = ours.iter().position(|(p, _)| p.frame == s.frame && p.trigger == b.trigger) {
                    matched[i] = true;
                }
                tally.lines.push(format!("    {what}: the exhaustion's breath (aware), taken from the recording (a bot's stamina is not recorded): heard as long as it plays"));
                continue;
            }
            tally.recorded += 1;
            let found = ours.iter().map(|(p, _)| p).enumerate().filter(|(i, p)| !matched[*i] && p.trigger == b.trigger).min_by_key(|(_, p)| (p.frame - s.frame).abs());
            let mut line = format!("    {what}");
            match found {
                Some((i, p)) if (p.frame - s.frame).abs() <= 3 => {
                    matched[i] = true;
                    if p.frame == s.frame {
                        tally.same_frame += 1;
                        line += ": said on its frame";
                    } else {
                        line += &format!(": said {} frames off", p.frame - s.frame);
                    }
                    let drawable = b.ignore_tags || compare(tags_of(*clip), p.tags);
                    if drawable {
                        tally.clip_drawable += 1;
                    }
                    line += &format!(", the clip {} (tags {:#06x} against {:#06x}; ours drew {})", if drawable { "drawable" } else { "NOT drawable" }, tags_of(*clip), p.tags, b.clips.get(p.clip).map_or("?", |c| c.name.as_str()));
                }
                Some((_, p)) => line += &format!(": NOT said then (ours nearest on frame {})", p.frame),
                None => line += ": NOT said by the rule",
            }
            // The levels: the clip's volume, the source's, the farthest. The
            // source's own figures are as the frame's last phrase of the bot
            // left it (two in a frame: the breath, then the grunt over it).
            let c = &b.clips[*clip];
            let (last_bank, last_clip) = recorded.iter().filter(|(o, ..)| o.frame == s.frame).last().map_or((*bank, *clip), |(_, lb, lc, _)| (*lb, *lc));
            let last = &banks[last_bank].clips[last_clip];
            let share = s.max / last.falloff;
            let level_ok = (s.volume - c.volume).abs() < 1e-3 && (s.base_volume - base_volume).abs() < 1e-3;
            let reach_ok = (share - 1.0).abs() < 1e-3 || (share - 0.98).abs() < 1e-3 || (s.occlusion < 1.0 && share < 1.0);
            let source_ok = (s.source_volume - s.base_volume * last.volume * s.occlusion).abs() < 1e-3;
            if level_ok {
                tally.level += 1;
            }
            if reach_ok {
                tally.reach += 1;
            }
            if source_ok {
                tally.source += 1;
            }
            if s.group == "ObservedPlayerSpeech" {
                tally.group += 1;
            }
            if *off < 0.3 {
                tally.placed += 1;
            }
            line += &format!(
                "; volume {} (the clip's {}), the source's {} (preset {base_volume}), farthest {:.2} = falloff {} x {share:.4} (occlusion {:.2}{}), pitch {}, {}, {off:.3} m from its head; the source's volume {:.4} ({} {:.4} x occlusion)",
                s.volume,
                c.volume,
                s.base_volume,
                s.max,
                last.falloff,
                s.occlusion,
                if last.name != c.name { format!(", as {} left the source", last.name) } else { String::new() },
                s.pitch,
                s.group,
                s.source_volume,
                if source_ok { "=" } else { "NOT" },
                s.base_volume * last.volume
            );
            tally.lines.push(line);
        }
        for (i, (p, cause)) in ours.iter().enumerate().filter(|(i, _)| !matched[*i]) {
            let b = &banks[p.bank];
            let frame = frames.get(&p.frame);
            let distance = frame.and_then(|f| Some(gap(f.camera?, f.bots.get(&setup.id).and_then(|b| b.head)?)));
            let falloff = b.clips.get(p.clip).map_or(0.0, |c| c.falloff);
            let beyond = distance.is_some_and(|d| d > falloff + 1.0);
            match cause {
                _ if beyond => tally.ours_out_of_reach += 1,
                Cause::Recorded => tally.ours_unrecorded += 1,
                Cause::Bleeding | Cause::BlastDeath => tally.ours_unknown += 1,
            }
            let _ = i;
            tally.lines.push(format!(
                "    frame {}: ours said phrase {} ({}) with none recorded{}{}",
                p.frame,
                p.trigger,
                b.clips.get(p.clip).map_or("?", |c| c.name.as_str()),
                match distance {
                    Some(d) if beyond => format!(": {d:.1} m from the ear, past its reach {:.0} m (the game plays nothing)", falloff + 1.0),
                    Some(d) => format!(", {d:.1} m from the ear"),
                    None => String::new(),
                },
                match cause {
                    Cause::Recorded => "",
                    Cause::Bleeding => " (a bleeding's sixteenth tick: the count's target goes by the damaging effects at work, not recorded)",
                    Cause::BlastDeath => " (a death by a blast: the part it reached last, not recorded, decides it; the head's is silent)",
                }
            ));
        }
    }
    for (id, rest) in owned {
        for (s, ..) in rest {
            tally.lines.push(format!("  frame {}: {} given to bot {id}, which has no set-up", s.frame, s.clip));
        }
    }
    // Hits on characters the world file does not follow (no set-up).
    let mut strangers: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
    for (frame, list) in hits {
        for hit in list.iter().filter(|h| !setups.iter().any(|s| s.profile == h.profile)) {
            strangers.entry(hit.profile.as_str()).or_default().push(*frame);
        }
    }
    for (profile, frames) in strangers {
        tally.lines.push(format!("  {} hits on {profile}, which the world file does not follow (frames {:?}): not compared", frames.len(), frames));
    }
    tally
}
