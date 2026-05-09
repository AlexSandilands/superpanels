//! `superpanels monitor` subcommand: manage `[[monitor]]` blocks in the config.

use anyhow::{Context, Result, anyhow, bail};
use superpanels_core::config::{Config, MonitorIdentifier, diagonal_to_mm, write_monitor_block};

pub(crate) fn monitor_configure_cmd(
    identifier: &str,
    stable_id: bool,
    mm: Option<&str>,
    diagonal: Option<&str>,
    aspect: Option<&str>,
) -> Result<()> {
    let physical_mm = resolve_physical_mm(mm, diagonal, aspect)?;

    let id = if stable_id {
        MonitorIdentifier::StableId(identifier.to_owned())
    } else {
        MonitorIdentifier::Name(identifier.to_owned())
    };

    let path = Config::default_path().context("locating config path")?;
    write_monitor_block(&path, &id, physical_mm).context("writing monitor block")?;
    println!(
        "wrote {field} = \"{identifier}\", physical_mm = [{w:.1}, {h:.1}] to {p}",
        field = if stable_id { "stable_id" } else { "name" },
        w = physical_mm[0],
        h = physical_mm[1],
        p = path.display(),
    );
    Ok(())
}

pub(crate) fn resolve_physical_mm(
    mm: Option<&str>,
    diagonal: Option<&str>,
    aspect: Option<&str>,
) -> Result<[f64; 2]> {
    match (mm, diagonal, aspect) {
        (Some(s), None, None) => parse_mm(s),
        (None, Some(d), Some(a)) => {
            let inches = parse_diagonal(d)?;
            let (aw, ah) = parse_aspect(a)?;
            Ok(diagonal_to_mm(inches, aw, ah))
        }
        (None, None, None) => bail!("provide one of `--mm WxH` or `--diagonal D --aspect W:H`"),
        _ => bail!("`--mm` is mutually exclusive with `--diagonal`/`--aspect`"),
    }
}

fn parse_mm(s: &str) -> Result<[f64; 2]> {
    let (w, h) = s
        .split_once('x')
        .ok_or_else(|| anyhow!("expected `WxH`, got `{s}`"))?;
    let w: f64 = w.parse().context("parsing mm width")?;
    let h: f64 = h.parse().context("parsing mm height")?;
    if !(w.is_finite() && w > 0.0 && h.is_finite() && h > 0.0) {
        bail!("mm values must be finite and > 0");
    }
    Ok([w, h])
}

fn parse_diagonal(s: &str) -> Result<f64> {
    let stripped = s
        .strip_suffix("in")
        .or_else(|| s.strip_suffix('"'))
        .unwrap_or(s);
    let v: f64 = stripped
        .parse()
        .with_context(|| format!("parsing diagonal `{s}`"))?;
    if v <= 0.0 || !v.is_finite() {
        bail!("diagonal must be > 0");
    }
    Ok(v)
}

fn parse_aspect(s: &str) -> Result<(u32, u32)> {
    let (w, h) = s
        .split_once(':')
        .ok_or_else(|| anyhow!("expected `W:H`, got `{s}`"))?;
    let w: u32 = w.parse().context("parsing aspect width")?;
    let h: u32 = h.parse().context("parsing aspect height")?;
    if w == 0 || h == 0 {
        bail!("aspect components must be > 0");
    }
    Ok((w, h))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests assert on Ok variant; failure is a test bug
mod tests {
    use super::{parse_aspect, parse_diagonal, parse_mm, resolve_physical_mm};

    // parse_mm

    #[test]
    fn parse_mm_happy_path_returns_pair() {
        // Arrange / Act
        let got = parse_mm("597x336").unwrap();

        // Assert
        assert!((got[0] - 597.0).abs() < f64::EPSILON);
        assert!((got[1] - 336.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_mm_missing_separator_returns_error() {
        // Act
        let result = parse_mm("597-336");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn parse_mm_non_numeric_returns_error() {
        // Act
        let result = parse_mm("abcxdef");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn parse_mm_zero_is_rejected() {
        // Act
        let result = parse_mm("0x336");

        // Assert
        assert!(result.is_err());
    }

    // parse_diagonal

    #[test]
    fn parse_diagonal_bare_number_parses() {
        // Act
        let got = parse_diagonal("27").unwrap();

        // Assert
        assert!((got - 27.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_diagonal_with_in_suffix_parses() {
        // Act
        let got = parse_diagonal("27in").unwrap();

        // Assert
        assert!((got - 27.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_diagonal_with_inch_quote_suffix_parses() {
        // Act
        let got = parse_diagonal("27\"").unwrap();

        // Assert
        assert!((got - 27.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_diagonal_zero_is_rejected() {
        // Act
        let result = parse_diagonal("0");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn parse_diagonal_negative_is_rejected() {
        // Act
        let result = parse_diagonal("-27");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn parse_diagonal_non_finite_is_rejected() {
        // Act
        let result = parse_diagonal("inf");

        // Assert
        assert!(result.is_err());
    }

    // parse_aspect

    #[test]
    fn parse_aspect_happy_path_returns_pair() {
        // Act
        let got = parse_aspect("16:9").unwrap();

        // Assert
        assert_eq!(got, (16, 9));
    }

    #[test]
    fn parse_aspect_missing_separator_returns_error() {
        // Act
        let result = parse_aspect("16x9");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn parse_aspect_zero_denominator_is_rejected() {
        // Act
        let result = parse_aspect("16:0");

        // Assert
        assert!(result.is_err());
    }

    // resolve_physical_mm — all four match arms

    #[test]
    fn resolve_physical_mm_arm_mm_only_returns_parsed_pair() {
        // Act
        let got = resolve_physical_mm(Some("597x336"), None, None).unwrap();

        // Assert
        assert!((got[0] - 597.0).abs() < f64::EPSILON);
        assert!((got[1] - 336.0).abs() < f64::EPSILON);
    }

    #[test]
    fn resolve_physical_mm_arm_mm_only_accepts_fractional_input() {
        // Act
        let got = resolve_physical_mm(Some("597.5x336.2"), None, None).unwrap();

        // Assert
        assert!((got[0] - 597.5).abs() < f64::EPSILON);
        assert!((got[1] - 336.2).abs() < f64::EPSILON);
    }

    #[test]
    fn resolve_physical_mm_arm_diagonal_and_aspect_returns_computed_pair() {
        // Arrange + Act — 27" 16:9 → ~597x336 mm.
        let got = resolve_physical_mm(None, Some("27in"), Some("16:9")).unwrap();

        // Assert
        assert!(
            (590.0..=605.0).contains(&got[0]),
            "width was {}, expected ~597",
            got[0]
        );
        assert!(
            (330.0..=345.0).contains(&got[1]),
            "height was {}, expected ~336",
            got[1]
        );
    }

    #[test]
    fn resolve_physical_mm_arm_neither_returns_help_error() {
        // Act
        let result = resolve_physical_mm(None, None, None);

        // Assert
        let err = result.unwrap_err().to_string();
        assert!(err.contains("--mm"), "msg was {err}");
        assert!(err.contains("--diagonal"), "msg was {err}");
    }

    #[test]
    fn resolve_physical_mm_arm_mm_with_diagonal_returns_mutex_error() {
        // Act
        let result = resolve_physical_mm(Some("597x336"), Some("27"), None);

        // Assert
        let err = result.unwrap_err().to_string();
        assert!(err.contains("mutually exclusive"), "msg was {err}");
    }
}
