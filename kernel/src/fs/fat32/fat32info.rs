use super::bpb::BootSector;
use crate::stack_trace;

#[derive(Copy, Clone, Default)]
pub struct FAT32Info {
    /// 备份引导扇区号。真正的引导扇区为0扇区
    pub bk_bootsector_id: usize,
    /// FSInfo 扇区号。用于存储和簇分配有关的信息
    pub fsinfo_sector_id: usize,
    /// FAT 表初始位置。这在引导扇区记录中对应保留扇区数量
    pub fat_start_sector: usize,
    /// 单个 FAT 表占用扇区数
    pub fat_sector_count: usize,
    /// FAT 表的个数
    pub fat_count: usize,
    /// 数据区起始位置。在挂载过程中计算出来
    pub data_start_sector: usize,
    /// 一个簇的扇区数。这个数为2的次幂
    pub sector_per_cluster: usize,
    /// 磁盘的总扇区数
    pub tot_sector_count: usize,
    /// 磁盘的总簇数，计算得出
    pub tot_cluster_count: usize,
    /// 根目录的簇号
    pub root_cluster_id: usize,
}

impl FAT32Info {
    pub fn new(bs: BootSector) -> Self {
        stack_trace!();
        let start_sector = (bs.BPB_ReservedSectorCount as usize)
            + (bs.BPB_NumFATs as usize) * (bs.BPB_FATsize32 as usize);
        let cluster_count =
            (bs.BPB_TotSector32 as usize - start_sector) / (bs.BPB_SectorPerCluster as usize);
        Self {
            bk_bootsector_id: bs.BPB_BkBootSec as usize,
            fsinfo_sector_id: bs.BPB_FSInfo as usize,
            fat_start_sector: bs.BPB_ReservedSectorCount as usize,
            fat_sector_count: bs.BPB_FATsize32 as usize,
            fat_count: bs.BPB_NumFATs as usize,
            data_start_sector: start_sector,
            sector_per_cluster: bs.BPB_SectorPerCluster as usize,
            tot_sector_count: bs.BPB_TotSector32 as usize,
            tot_cluster_count: cluster_count as usize,
            root_cluster_id: bs.BPB_RootCluster as usize,
        }
    }

    pub fn cid_to_sid(&self, cluster_id: usize) -> Option<usize> {
        stack_trace!();
        if cluster_id < 2 {
            return None;
        }
        let ret = (cluster_id - 2) * self.sector_per_cluster + self.data_start_sector;
        if ret >= self.tot_sector_count {
            return None;
        }
        Some(ret)
    }
}
