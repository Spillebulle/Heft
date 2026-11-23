//! One size formatter for the whole application, so the same folder never
//! reads two ways on one screen.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Units {
    /// Powers of 1000: kB, MB, GB. GNOME, macOS, drive labels.
    Decimal,
    /// Powers of 1024 labelled KiB, MiB, GiB.
    Binary,
    /// Powers of 1024 labelled KB, MB, GB: what Windows Explorer shows.
    Windows,
}

impl Units {
    /// What this platform's own file manager shows.
    pub fn platform_default() -> Units {
        if cfg!(windows) {
            Units::Windows
        } else {
            Units::Decimal
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Units::Decimal => "decimal",
            Units::Binary => "binary",
            Units::Windows => "windows",
        }
    }

    pub fn from_key(k: &str) -> Option<Units> {
        match k {
            "decimal" => Some(Units::Decimal),
            "binary" => Some(Units::Binary),
            "windows" => Some(Units::Windows),
            _ => None,
        }
    }
}

pub fn format_size(bytes: u64, units: Units) -> String {
    let (base, labels): (f64, [&str; 7]) = match units {
        Units::Decimal => (1000.0, ["B", "kB", "MB", "GB", "TB", "PB", "EB"]),
        Units::Binary => (1024.0, ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"]),
        Units::Windows => (1024.0, ["B", "KB", "MB", "GB", "TB", "PB", "EB"]),
    };
    if (bytes as f64) < base {
        return format!("{bytes} B");
    }
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= base && i < labels.len() - 1 {
        v /= base;
        i += 1;
    }
    if v >= 100.0 {
        format!("{v:.0} {}", labels[i])
    } else if v >= 10.0 {
        format!("{v:.1} {}", labels[i])
    } else {
        format!("{v:.2} {}", labels[i])
    }
}

/// A count with thousands separators: 1,204,331.
pub fn format_count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Elapsed time for the status bar: "850 ms", "6.2 s", "3 min 4 s".
pub fn format_duration(d: std::time::Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        format!("{:.1} s", d.as_secs_f64())
    } else {
        let s = d.as_secs();
        format!("{} min {} s", s / 60, s % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(format_size(999, Units::Decimal), "999 B");
        assert_eq!(format_size(1500, Units::Decimal), "1.50 kB");
        assert_eq!(format_size(1536, Units::Binary), "1.50 KiB");
        assert_eq!(format_size(412_300_000_000, Units::Decimal), "412 GB");
        assert_eq!(format_size(10 * 1024 * 1024, Units::Windows), "10.0 MB");
    }

    #[test]
    fn counts() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(1204331), "1,204,331");
        assert_eq!(format_count(100), "100");
    }
}
