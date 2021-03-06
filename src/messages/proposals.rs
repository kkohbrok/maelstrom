use crate::ciphersuite::*;
use crate::codec::*;
use crate::framing::*;
use crate::key_packages::*;
use crate::tree::index::LeafIndex;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum ProposalType {
    Invalid = 0,
    Add = 1,
    Update = 2,
    Remove = 3,
    Default = 255,
}

impl From<u8> for ProposalType {
    fn from(value: u8) -> Self {
        match value {
            0 => ProposalType::Invalid,
            1 => ProposalType::Add,
            2 => ProposalType::Update,
            3 => ProposalType::Remove,
            _ => ProposalType::Default,
        }
    }
}

impl Codec for ProposalType {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        (*self as u8).encode(buffer)?;
        Ok(())
    }
    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     Ok(ProposalType::from(u8::decode(cursor)?))
    // }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq, Clone)]
pub enum Proposal {
    Add(AddProposal),
    Update(UpdateProposal),
    Remove(RemoveProposal),
}

impl Proposal {
    pub fn to_proposal_id(&self, ciphersuite: &Ciphersuite) -> ProposalID {
        ProposalID::from_proposal(ciphersuite, self)
    }
    pub fn as_add(&self) -> Option<AddProposal> {
        match self {
            Proposal::Add(add_proposal) => Some(add_proposal.clone()),
            _ => None,
        }
    }
    pub fn as_update(&self) -> Option<UpdateProposal> {
        match self {
            Proposal::Update(update_proposal) => Some(update_proposal.clone()),
            _ => None,
        }
    }
    pub fn as_remove(&self) -> Option<RemoveProposal> {
        match self {
            Proposal::Remove(remove_proposal) => Some(remove_proposal.clone()),
            _ => None,
        }
    }
}

impl Codec for Proposal {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        match self {
            Proposal::Add(add) => {
                ProposalType::Add.encode(buffer)?;
                add.encode(buffer)?;
            }
            Proposal::Update(update) => {
                ProposalType::Update.encode(buffer)?;
                update.encode(buffer)?;
            }
            Proposal::Remove(remove) => {
                ProposalType::Remove.encode(buffer)?;
                remove.encode(buffer)?;
            }
        }
        Ok(())
    }
    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let proposal_type = ProposalType::from(u8::decode(cursor)?);
    //     match proposal_type {
    //         ProposalType::Add => Ok(Proposal::Add(AddProposal::decode(cursor)?)),
    //         ProposalType::Update => Ok(Proposal::Update(UpdateProposal::decode(cursor)?)),
    //         ProposalType::Remove => Ok(Proposal::Remove(RemoveProposal::decode(cursor)?)),
    //         _ => Err(CodecError::DecodingError),
    //     }
    // }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ProposalID {
    value: Vec<u8>,
}

impl ProposalID {
    pub fn from_proposal(ciphersuite: &Ciphersuite, proposal: &Proposal) -> Self {
        let encoded = proposal.encode_detached().unwrap();
        let value = ciphersuite.hash(&encoded);
        Self { value }
    }
}

impl Codec for ProposalID {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        encode_vec(VecSize::VecU8, buffer, &self.value)?;
        Ok(())
    }
    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let value = decode_vec(VecSize::VecU8, cursor)?;
    //     Ok(ProposalID { value })
    // }
}

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct ShortProposalID([u8; 32]);

impl ShortProposalID {
    pub fn from_proposal_id(proposal_id: &ProposalID) -> ShortProposalID {
        let mut inner = [0u8; 32];
        inner.copy_from_slice(&proposal_id.value[..32]);
        ShortProposalID(inner)
    }
}

impl Codec for ShortProposalID {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        encode_vec(VecSize::VecU8, buffer, &self.0)?;
        Ok(())
    }
    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let value = decode_vec(VecSize::VecU8, cursor)?;
    //     let mut inner = [0u8; 32];
    //     inner.copy_from_slice(&value[..32]);
    //     Ok(ShortProposalID(inner))
    // }
}

#[derive(Clone)]
pub struct QueuedProposal {
    pub proposal: Proposal,
    pub sender: Sender,
    pub own_kpb: Option<KeyPackageBundle>,
}

impl QueuedProposal {
    pub fn new(proposal: Proposal, sender: LeafIndex, own_kpb: Option<KeyPackageBundle>) -> Self {
        Self {
            proposal,
            sender: Sender::member(sender),
            own_kpb,
        }
    }
}

impl Codec for QueuedProposal {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        self.proposal.encode(buffer)?;
        self.sender.encode(buffer)?;
        self.own_kpb.encode(buffer)?;
        Ok(())
    }
    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let proposal = Proposal::decode(cursor)?;
    //     let sender = Sender::decode(cursor)?;
    //     let own_kpb = Option::<KeyPackageBundle>::decode(cursor)?;
    //     Ok(QueuedProposal {
    //         proposal,
    //         sender,
    //         own_kpb,
    //     })
    // }
}

#[derive(Default, Clone)]
pub struct ProposalQueue {
    tuples: HashMap<ShortProposalID, (ProposalID, QueuedProposal)>,
}

impl ProposalQueue {
    pub fn new() -> Self {
        ProposalQueue {
            tuples: HashMap::new(),
        }
    }
    pub fn add(&mut self, queued_proposal: QueuedProposal, ciphersuite: &Ciphersuite) {
        let pi = ProposalID::from_proposal(ciphersuite, &queued_proposal.proposal);
        let spi = ShortProposalID::from_proposal_id(&pi);
        self.tuples.entry(spi).or_insert((pi, queued_proposal));
    }
    pub fn get(&self, proposal_id: &ProposalID) -> Option<&(ProposalID, QueuedProposal)> {
        let spi = ShortProposalID::from_proposal_id(&proposal_id);
        self.tuples.get(&spi)
    }
    pub fn get_commit_lists(&self, ciphersuite: &Ciphersuite) -> ProposalIDList {
        let mut updates = vec![];
        let mut removes = vec![];
        let mut adds = vec![];
        for (_spi, p) in self.tuples.values() {
            match p.proposal {
                Proposal::Update(_) => updates.push(p.proposal.to_proposal_id(ciphersuite)),
                Proposal::Remove(_) => removes.push(p.proposal.to_proposal_id(ciphersuite)),
                Proposal::Add(_) => adds.push(p.proposal.to_proposal_id(ciphersuite)),
            }
        }
        ProposalIDList {
            updates,
            removes,
            adds,
        }
    }
}

impl Codec for ProposalQueue {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        self.tuples.encode(buffer)?;
        Ok(())
    }
    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let tuples = HashMap::<ShortProposalID, (ProposalID, QueuedProposal)>::decode(cursor)?;
    //     Ok(ProposalQueue { tuples })
    // }
}

#[derive(Clone)]
pub struct ProposalIDList {
    pub updates: Vec<ProposalID>,
    pub removes: Vec<ProposalID>,
    pub adds: Vec<ProposalID>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct AddProposal {
    pub key_package: KeyPackage,
}

impl Codec for AddProposal {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        self.key_package.encode(buffer)?;
        Ok(())
    }
    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let key_package = KeyPackage::decode(cursor)?;
    //     Ok(AddProposal { key_package })
    // }
}

#[derive(Debug, PartialEq, Clone)]
pub struct UpdateProposal {
    pub key_package: KeyPackage,
}

impl Codec for UpdateProposal {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        self.key_package.encode(buffer)?;
        Ok(())
    }
    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let key_package = KeyPackage::decode(cursor)?;
    //     Ok(UpdateProposal { key_package })
    // }
}

#[derive(Debug, PartialEq, Clone)]
pub struct RemoveProposal {
    pub removed: u32,
}

impl Codec for RemoveProposal {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        self.removed.encode(buffer)?;
        Ok(())
    }
    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let removed = u32::decode(cursor)?;
    //     Ok(RemoveProposal { removed })
    // }
}
