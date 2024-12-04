use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use typenum::Unsigned;

pub trait ConsensusSpec: 'static + Default + Sync + Send + Clone + Debug + PartialEq {
    type MaxProposerSlashings: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;
    type MaxAttesterSlashings: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;
    type MaxAttestations: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;
    type MaxDeposits: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;
    type MaxVoluntaryExits: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;
    type MaxBlsToExecutionChanged: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;
    type MaxBlobKzgCommitments: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;
    type MaxWithdrawals: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;
    type MaxValidatorsPerCommitee: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;
    type SlotsPerEpoch: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;
    type EpochsPerSyncCommiteePeriod: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;
    type SyncCommitteeSize: Unsigned + Default + Debug + Sync + Send + Clone + PartialEq;

    fn slots_per_epoch() -> u64 {
        Self::SlotsPerEpoch::to_u64()
    }

    fn epochs_per_sync_commitee_period() -> u64 {
        Self::EpochsPerSyncCommiteePeriod::to_u64()
    }

    fn slots_per_sync_commitee_period() -> u64 {
        Self::slots_per_epoch() * Self::epochs_per_sync_commitee_period()
    }

    fn sync_commitee_size() -> u64 {
        Self::SyncCommitteeSize::to_u64()
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct MainnetConsensusSpec;

impl ConsensusSpec for MainnetConsensusSpec {
    type MaxProposerSlashings = typenum::U16;
    type MaxAttesterSlashings = typenum::U2;
    type MaxAttestations = typenum::U128;
    type MaxDeposits = typenum::U16;
    type MaxVoluntaryExits = typenum::U16;
    type MaxBlsToExecutionChanged = typenum::U16;
    type MaxBlobKzgCommitments = typenum::U4096;
    type MaxWithdrawals = typenum::U16;
    type MaxValidatorsPerCommitee = typenum::U2048;
    type SlotsPerEpoch = typenum::U32;
    type EpochsPerSyncCommiteePeriod = typenum::U256;
    type SyncCommitteeSize = typenum::U512;
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct GnosisConsensusSpec;
// Here is the modified version for Gnosis Chain
impl ConsensusSpec for GnosisConsensusSpec {
    type MaxProposerSlashings = typenum::U16;
    type MaxAttesterSlashings = typenum::U2;
    type MaxAttestations = typenum::U128;
    type MaxDeposits = typenum::U16;
    type MaxVoluntaryExits = typenum::U16;
    type MaxBlsToExecutionChanged = typenum::U16;
    type MaxBlobKzgCommitments = typenum::U4096;
    type MaxWithdrawals = typenum::U8;
    type MaxValidatorsPerCommitee = typenum::U2048;
    type SlotsPerEpoch = typenum::U16;
    type EpochsPerSyncCommiteePeriod = typenum::U512;
    type SyncCommitteeSize = typenum::U512;
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct MinimalConsensusSpec;

impl ConsensusSpec for MinimalConsensusSpec {
    type MaxProposerSlashings = typenum::U16;
    type MaxAttesterSlashings = typenum::U2;
    type MaxAttestations = typenum::U128;
    type MaxDeposits = typenum::U16;
    type MaxVoluntaryExits = typenum::U16;
    type MaxBlsToExecutionChanged = typenum::U16;
    type MaxBlobKzgCommitments = typenum::U4096;
    type MaxWithdrawals = typenum::U16;
    type MaxValidatorsPerCommitee = typenum::U2048;
    type SlotsPerEpoch = typenum::U8;
    type EpochsPerSyncCommiteePeriod = typenum::U8;
    type SyncCommitteeSize = typenum::U32;
}
