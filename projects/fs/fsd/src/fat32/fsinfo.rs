pub const SIG_START: u32 = 0x41615252;
pub const SIG_END: u32 = 0xAA550000;

/// FSInfo sector offset
enum FSInfoOffset {
    /// FSInfo sector header signature, value is [SIG_START]
    LeadSig = 0,
    /// Reserved field
    Reserved1 = 4,
    /// Latest free cluster count
    ///
    /// If 0xFFFFFFFF, the free cluster count is unknown and needs to be recalculated
    FreeCount = 488,
    /// Last cluster number allocated by the driver
    ///
    /// If 0xFFFFFFFF, the search needs to start from cluster 2
    NxtFree = 492,
    /// Reserved field
    Reserved2 = 496,
    /// FSInfo sector trailer signature, value is [SIG_END]
    TrailSig = 508,
    /// End
    End = 512,
}

impl FSInfoOffset {
    /// FSInfo sector header signature, value is [SIG_START]
    pub fn lead_sig(sector: &[u8]) -> u32 {
        u32::from_le_bytes(section!(sector, LeadSig, Reserved1))
    }

    /// Latest free cluster count
    pub fn free_count(sector: &[u8]) -> u32 {
        u32::from_le_bytes(section!(sector, FreeCount, NxtFree))
    }

    /// Last cluster number allocated by the driver
    pub fn next_free(sector: &[u8]) -> u32 {
        u32::from_le_bytes(section!(sector, NxtFree, Reserved2))
    }

    /// FSInfo sector trailer signature, value is [SIG_END]
    pub fn trail_sig(sector: &[u8]) -> u32 {
        u32::from_le_bytes(section!(sector, TrailSig, End))
    }

    fn split(sector: &[u8], start: Self, end: Self) -> &[u8] {
        &sector[start as usize..end as usize]
    }
}
