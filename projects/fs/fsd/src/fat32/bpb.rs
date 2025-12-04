use alloc::string::String;

pub enum BootSectorOffset {
    /// Ignore this field
    JmpBoot = 0,
    /// Ignore this field
    OEMName = 3,
    /// Ignore this field
    DrvNum = 64,
    /// Reserved field
    Reserved1 = 65,
    /// Extended boot signature, used to indicate that the following 3 fields are available
    BootSig = 66,
    /// Ignore this field
    VolID = 67,
    /// Volume label
    ///
    /// This field must match the 11-byte volume label in the root directory
    VolLab = 71,
    /// File system type
    ///
    /// This field can be one of FAT12, FAT16, or FAT32
    FilSysType = 82,
}

impl BootSectorOffset {
    pub fn boot_sig(sector: &[u8]) -> u8 {
        u8::from_le_bytes(section!(sector, BootSig, VolID))
    }

    pub fn vol_lab(sector: &[u8]) -> String {
        String::from_utf8(section!(sector, VolLab, FilSysType)).unwrap_or_default()
    }

    fn split(sector: &[u8], start: Self, end: Self) -> &[u8] {
        &sector[start as usize..end as usize]
    }
}

/// BIOS Parameter Block (BPB) offsets
pub enum BPBOffset {
    /// Bytes per sector
    ///
    /// Valid values are: 512, 1024, 2048, or 4096
    BytsPerSec = 11,
    /// Sectors per cluster
    ///
    /// Value must be a power of 2
    SecPerClus = 13,
    /// Number of reserved sectors in the reserved region
    RsvdSecCnt = 14,
    /// Number of FAT tables in this volume, usually 2
    NumFATs = 16,
    /// For FAT32, this field must be 0
    RootEntCnt = 17,
    /// For FAT32, this field must be 0
    TotSec16 = 19,
    /// Ignore this field
    Media = 21,
    /// For FAT32, this field must be 0
    FATSz16 = 22,
    /// Ignore this field
    SecPerTrk = 24,
    /// Ignore this field
    NumHeads = 26,
    /// Number of hidden sectors before this FAT volume
    HiddSec = 28,
    /// Total number of sectors in the volume
    TotSec32 = 32,
    /// Number of sectors in one FAT table
    FATSz32 = 36,
    /// Bits 0-3: Active FAT table, only valid when mirroring is disabled
    ///
    /// Bit 7: 0 means FAT is mirrored to all FAT tables in real-time; 1 means only one FAT table is active
    ExtFlags = 40,
    /// FAT32 version number
    ///
    /// High byte is major version, low byte is minor version
    FSVer = 42,
    /// Cluster number of the first cluster of the root directory
    RootClus = 44,
    /// Number of sectors occupied by the FAT32 FSINFO structure in the reserved region
    FSInfo = 48,
    /// Ignore this field
    BkBootSec = 50,
    /// Reserved field
    Reserved = 52,
}

impl BPBOffset {
    /// Bytes per sector
    pub fn bytes_per_sector(sector: &[u8]) -> u16 {
        u16::from_le_bytes(section!(sector, BytsPerSec, SecPerClus))
    }

    /// Sectors per cluster
    pub fn sector_per_cluster(sector: &[u8]) -> u8 {
        u8::from_le_bytes(section!(sector, SecPerClus, RsvdSecCnt))
    }

    /// Number of reserved sectors in the reserved region
    pub fn reserved_sectors(sector: &[u8]) -> u16 {
        u16::from_le_bytes(section!(sector, RsvdSecCnt, NumFATs))
    }

    /// Number of FAT tables
    pub fn fats_number(sector: &[u8]) -> u8 {
        u8::from_le_bytes(section!(sector, NumFATs, RootEntCnt))
    }

    /// Total number of sectors in the volume
    pub fn total_sectors(sector: &[u8]) -> u32 {
        u32::from_le_bytes(section!(sector, TotSec32, FATSz32))
    }

    /// Number of sectors in one FAT table
    pub fn fat_size(sector: &[u8]) -> u32 {
        u32::from_le_bytes(section!(sector, FATSz32, ExtFlags))
    }

    /// Extended flags
    pub fn extend_flags(sector: &[u8]) -> u16 {
        u16::from_le_bytes(section!(sector, ExtFlags, FSVer))
    }

    /// Cluster number of the first cluster of the root directory
    pub fn root_cluster(sector: &[u8]) -> u32 {
        u32::from_le_bytes(section!(sector, RootClus, FSInfo))
    }

    /// Number of sectors occupied by the FAT32 FSINFO structure in the reserved region
    pub fn fs_info(sector: &[u8]) -> u16 {
        u16::from_le_bytes(section!(sector, FSInfo, BkBootSec))
    }

    fn split(sector: &[u8], start: Self, end: Self) -> &[u8] {
        &sector[start as usize..end as usize]
    }
}
