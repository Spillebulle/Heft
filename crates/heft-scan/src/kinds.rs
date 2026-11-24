//! What kind of file a name is, by extension. Drives the treemap's colours
//! and the File types view. Deliberately coarse: a dozen kinds a person can
//! hold in their head, not a MIME database.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FileKind {
    Video,
    Audio,
    Image,
    Archive,
    DiskImage,
    Document,
    Code,
    Build,
    Executable,
    Data,
    Font,
    Other,
}

impl FileKind {
    pub const ALL: [FileKind; 12] = [
        FileKind::Video,
        FileKind::Audio,
        FileKind::Image,
        FileKind::Archive,
        FileKind::DiskImage,
        FileKind::Document,
        FileKind::Code,
        FileKind::Build,
        FileKind::Executable,
        FileKind::Data,
        FileKind::Font,
        FileKind::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FileKind::Video => "Video",
            FileKind::Audio => "Audio",
            FileKind::Image => "Images",
            FileKind::Archive => "Archives",
            FileKind::DiskImage => "Disk images",
            FileKind::Document => "Documents",
            FileKind::Code => "Source code",
            FileKind::Build => "Build output",
            FileKind::Executable => "Programs and libraries",
            FileKind::Data => "Data and databases",
            FileKind::Font => "Fonts",
            FileKind::Other => "Other",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// The lowercase extension of a file name, without the dot, if it has one
/// that looks like an extension (not a dotfile, not absurdly long).
pub fn extension(name: &[u8]) -> Option<String> {
    let dot = name.iter().rposition(|&b| b == b'.')?;
    if dot == 0 || dot + 1 == name.len() || name.len() - dot > 12 {
        return None;
    }
    let ext = &name[dot + 1..];
    if !ext
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
    {
        return None;
    }
    Some(String::from_utf8_lossy(ext).to_ascii_lowercase())
}

pub fn kind_of(name: &[u8]) -> FileKind {
    match extension(name) {
        Some(e) => kind_of_ext(&e),
        None => FileKind::Other,
    }
}

pub fn kind_of_ext(e: &str) -> FileKind {
    use FileKind::*;
    match e {
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "flv" | "mpg" | "mpeg" | "ts"
        | "m2ts" | "vob" | "3gp" => Video,
        "mp3" | "flac" | "wav" | "ogg" | "opus" | "m4a" | "aac" | "wma" | "aiff" | "alac"
        | "mid" => Audio,
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tif" | "tiff" | "heic" | "heif"
        | "avif" | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" | "psd" | "kra" | "xcf"
        | "svg" | "ico" | "exr" | "hdr" => Image,
        "zip" | "7z" | "rar" | "tar" | "gz" | "tgz" | "xz" | "bz2" | "zst" | "lz4" | "lzma"
        | "cab" | "jar" | "deb" | "rpm" | "apk" | "nupkg" | "whl" | "crate" => Archive,
        "iso" | "img" | "vhd" | "vhdx" | "vmdk" | "qcow2" | "vdi" | "dmg" | "wim" | "esd"
        | "appimage" => DiskImage,
        "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "xls" | "xlsx" | "ods" | "csv"
        | "tsv" | "ppt" | "pptx" | "odp" | "epub" | "mobi" | "tex" => Document,
        "rs" | "c" | "h" | "cpp" | "hpp" | "cc" | "cs" | "java" | "kt" | "py" | "js" | "mjs"
        | "ts_" | "tsx" | "jsx" | "go" | "rb" | "php" | "swift" | "lua" | "sh" | "ps1" | "html"
        | "css" | "scss" | "vue" | "svelte" | "toml" | "yaml" | "yml" | "json" | "xml" | "map" => {
            Code
        }
        "o" | "obj" | "rlib" | "rmeta" | "pyc" | "class" | "pdb" | "bc" | "ilk" | "pch" | "gch"
        | "lib" | "a" | "d" | "timestamp" => Build,
        "exe" | "dll" | "so" | "sys" | "msi" | "msix" | "appx" | "bin" | "elf" | "ko" | "efi" => {
            Executable
        }
        "db" | "sqlite" | "sqlite3" | "mdb" | "ldb" | "log" | "dat" | "pak" | "vpk" | "bsa"
        | "ba2" | "pck" | "idx" | "parquet" | "npy" | "h5" | "pkl" | "pt" | "safetensors"
        | "gguf" | "onnx" | "ckpt" | "blend" | "fbx" | "uasset" => Data,
        "ttf" | "otf" | "woff" | "woff2" | "ttc" => Font,
        _ => Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions() {
        assert_eq!(extension(b"Movie.MKV").as_deref(), Some("mkv"));
        assert_eq!(extension(b".bashrc"), None);
        assert_eq!(extension(b"noext"), None);
        assert_eq!(extension(b"trailing."), None);
        assert_eq!(kind_of(b"a.tar.gz"), FileKind::Archive);
        assert_eq!(kind_of(b"photo.jpeg"), FileKind::Image);
        assert_eq!(kind_of(b"libfoo-abc.rlib"), FileKind::Build);
    }
}
