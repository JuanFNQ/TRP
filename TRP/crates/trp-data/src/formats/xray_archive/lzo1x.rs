//! Unpacking LZO1X.

use super::*;

/// Unpacks LZO1X to `size` bytes.
pub fn unpack_lzo1x(packed: &[u8], size: usize) -> Result<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(size);
    let mut ip = 0usize;
    let next = |ip: &mut usize| -> Result<usize> {
        let byte = *packed.get(*ip).ok_or_else(|| anyhow!("the packed bytes end early"))?;
        *ip += 1;
        Ok(byte as usize)
    };
    // A count past a code's own bits: every zero byte 255 more, then the
    // first that is not.
    let long = |ip: &mut usize| -> Result<usize> {
        let mut count = 0;
        while *packed.get(*ip).ok_or_else(|| anyhow!("the packed bytes end early"))? == 0 {
            count += 255;
            *ip += 1;
        }
        Ok(count + next(ip)?)
    };
    let literals = |out: &mut Vec<u8>, ip: &mut usize, count: usize| -> Result<()> {
        let run = packed.get(*ip..*ip + count).ok_or_else(|| anyhow!("a run of {count} bytes goes past the packed bytes"))?;
        out.extend_from_slice(run);
        *ip += count;
        Ok(())
    };
    let copy = |out: &mut Vec<u8>, distance: usize, count: usize| -> Result<()> {
        ensure!(distance >= 1 && distance <= out.len(), "a match {distance} bytes back, of {} made", out.len());
        let from = out.len() - distance;
        for k in 0..count {
            out.push(out[from + k]);
        }
        Ok(())
    };
    enum Step {
        Literals,
        AfterLiterals,
        Match(usize),
        Trailing(usize),
    }
    let mut step = Step::Literals;
    if packed.first().is_some_and(|first| *first > 17) {
        let t = next(&mut ip)? - 17;
        if t < 4 {
            step = Step::Trailing(t);
        } else {
            literals(&mut out, &mut ip, t)?;
            step = Step::AfterLiterals;
        }
    }
    loop {
        ensure!(out.len() <= size, "more than the {size} bytes listed");
        step = match step {
            Step::Literals => {
                let mut t = next(&mut ip)?;
                if t >= 16 {
                    Step::Match(t)
                } else {
                    if t == 0 {
                        t = 15 + long(&mut ip)?;
                    }
                    literals(&mut out, &mut ip, t + 3)?;
                    Step::AfterLiterals
                }
            }
            // Just after a run of literals, a code under 16 is a match of
            // three bytes from beyond the short reach.
            Step::AfterLiterals => {
                let t = next(&mut ip)?;
                if t >= 16 {
                    Step::Match(t)
                } else {
                    let distance = 1 + 0x0800 + (t >> 2) + (next(&mut ip)? << 2);
                    copy(&mut out, distance, 3)?;
                    Step::Trailing(packed[ip - 2] as usize & 3)
                }
            }
            Step::Match(t) => {
                if t >= 64 {
                    let distance = 1 + ((t >> 2) & 7) + (next(&mut ip)? << 3);
                    copy(&mut out, distance, (t >> 5) + 1)?;
                } else if t >= 32 {
                    let mut count = t & 31;
                    if count == 0 {
                        count = 31 + long(&mut ip)?;
                    }
                    let (low, high) = (next(&mut ip)?, next(&mut ip)?);
                    copy(&mut out, 1 + (low >> 2) + (high << 6), count + 2)?;
                } else if t >= 16 {
                    let mut count = t & 7;
                    if count == 0 {
                        count = 7 + long(&mut ip)?;
                    }
                    let (low, high) = (next(&mut ip)?, next(&mut ip)?);
                    let distance = ((t & 8) << 11) + (low >> 2) + (high << 6);
                    // A distance of nothing ends the stream.
                    if distance == 0 {
                        break;
                    }
                    copy(&mut out, distance + 0x4000, count + 2)?;
                } else {
                    let distance = 1 + (t >> 2) + (next(&mut ip)? << 2);
                    copy(&mut out, distance, 2)?;
                }
                Step::Trailing(packed[ip - 2] as usize & 3)
            }
            // The last two bits of a match's code: so many literals after
            // it, and then a code that is a match whatever its value.
            Step::Trailing(0) => Step::Literals,
            Step::Trailing(t) => {
                literals(&mut out, &mut ip, t)?;
                Step::Match(next(&mut ip)?)
            }
        };
    }
    if out.len() != size {
        bail!("{} bytes unpacked where {size} are listed", out.len());
    }
    Ok(out)
}
