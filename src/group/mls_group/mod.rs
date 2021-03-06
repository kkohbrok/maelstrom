// maelstrom
// Copyright (C) 2020 Raphael Robert
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see http://www.gnu.org/licenses/.

mod api;
mod apply_commit;
mod create_commit;
mod new_from_welcome;

use crate::ciphersuite::*;
use crate::codec::*;
use crate::framing::*;
use crate::group::*;
use crate::key_packages::*;
use crate::messages::{proposals::*, *};
use crate::schedule::*;
use crate::tree::{astree::*, index::*, node::*, *};

pub use api::*;
use apply_commit::*;
use create_commit::*;
use new_from_welcome::*;

use std::cell::{Ref, RefCell};

pub struct MlsGroup {
    ciphersuite: Ciphersuite,
    group_context: GroupContext,
    generation: u32,
    epoch_secrets: EpochSecrets,
    astree: RefCell<ASTree>,
    tree: RefCell<RatchetTree>,
    interim_transcript_hash: Vec<u8>,
}

impl Api for MlsGroup {
    fn new(id: &[u8], ciphersuite: Ciphersuite, key_package_bundle: KeyPackageBundle) -> MlsGroup {
        let group_id = GroupId { value: id.to_vec() };
        let epoch_secrets = EpochSecrets::new();
        let astree = ASTree::new(&epoch_secrets.application_secret, LeafIndex::from(1u32));
        let (private_key, key_package) = (
            key_package_bundle.private_key,
            key_package_bundle.key_package,
        );
        let kpb = KeyPackageBundle::from_values(key_package, private_key);
        let tree = RatchetTree::new(ciphersuite, kpb);
        let group_context = GroupContext {
            group_id,
            epoch: GroupEpoch(0),
            tree_hash: tree.compute_tree_hash(),
            confirmed_transcript_hash: vec![],
        };
        let interim_transcript_hash = vec![];
        MlsGroup {
            ciphersuite,
            group_context,
            generation: 0,
            epoch_secrets,
            astree: RefCell::new(astree),
            tree: RefCell::new(tree),
            interim_transcript_hash,
        }
    }
    // Join a group from a welcome message
    fn new_from_welcome(
        welcome: Welcome,
        nodes_option: Option<Vec<Option<Node>>>,
        kpb: KeyPackageBundle,
    ) -> Result<Self, WelcomeError> {
        new_from_welcome(welcome, nodes_option, kpb)
    }

    // Create handshake messages
    fn create_add_proposal(
        &self,
        aad: &[u8],
        signature_key: &SignaturePrivateKey,
        joiner_key_package: KeyPackage,
    ) -> (MLSPlaintext, Proposal) {
        let add_proposal = AddProposal {
            key_package: joiner_key_package,
        };
        let proposal = Proposal::Add(add_proposal);
        let content = MLSPlaintextContentType::Proposal(proposal.clone());
        let mls_plaintext = MLSPlaintext::new(
            &self.ciphersuite,
            self.get_sender_index(),
            aad,
            content,
            signature_key,
            &self.get_context(),
        );
        (mls_plaintext, proposal)
    }
    fn create_update_proposal(
        &self,
        aad: &[u8],
        signature_key: &SignaturePrivateKey,
        key_package: KeyPackage,
    ) -> (MLSPlaintext, Proposal) {
        let update_proposal = UpdateProposal { key_package };
        let proposal = Proposal::Update(update_proposal);
        let content = MLSPlaintextContentType::Proposal(proposal.clone());
        let mls_plaintext = MLSPlaintext::new(
            &self.ciphersuite,
            self.get_sender_index(),
            aad,
            content,
            signature_key,
            &self.get_context(),
        );
        (mls_plaintext, proposal)
    }
    fn create_remove_proposal(
        &self,
        aad: &[u8],
        signature_key: &SignaturePrivateKey,
        removed_index: LeafIndex,
    ) -> (MLSPlaintext, Proposal) {
        let remove_proposal = RemoveProposal {
            removed: removed_index.into(),
        };
        let proposal = Proposal::Remove(remove_proposal);
        let content = MLSPlaintextContentType::Proposal(proposal.clone());
        let mls_plaintext = MLSPlaintext::new(
            &self.ciphersuite,
            self.get_sender_index(),
            aad,
            content,
            signature_key,
            &self.get_context(),
        );
        (mls_plaintext, proposal)
    }
    fn create_commit(
        &self,
        aad: &[u8],
        signature_key: &SignaturePrivateKey,
        key_package_bundle: KeyPackageBundle,
        proposals: Vec<(Sender, Proposal)>,
        own_key_packages: Vec<KeyPackageBundle>,
        force_self_update: bool,
    ) -> CreateCommitResult {
        create_commit(
            self,
            aad,
            signature_key,
            key_package_bundle,
            proposals,
            own_key_packages,
            force_self_update,
        )
    }

    // Apply a Commit message
    fn apply_commit(
        &mut self,
        mls_plaintext: MLSPlaintext,
        proposals: Vec<(Sender, Proposal)>,
        own_key_packages: Vec<KeyPackageBundle>,
    ) -> Result<(), ApplyCommitError> {
        apply_commit(self, mls_plaintext, proposals, own_key_packages)
    }

    // Create application message
    fn create_application_message(
        &self,
        aad: &[u8],
        msg: &[u8],
        signature_key: &SignaturePrivateKey,
    ) -> MLSPlaintext {
        let content = MLSPlaintextContentType::Application(msg.to_vec());
        MLSPlaintext::new(
            &self.ciphersuite,
            self.get_sender_index(),
            aad,
            content,
            signature_key,
            &self.get_context(),
        )
    }

    // Encrypt/Decrypt MLS message
    fn encrypt(&mut self, mls_plaintext: MLSPlaintext) -> MLSCiphertext {
        let mut astree = self.astree.borrow_mut();
        let generation = astree.get_generation(mls_plaintext.sender.sender);
        let application_secrets = astree
            .get_secret(&self.ciphersuite, mls_plaintext.sender.sender, generation)
            .unwrap();
        MLSCiphertext::new_from_plaintext(&mls_plaintext, &self, generation, &application_secrets)
    }

    fn decrypt(&mut self, mls_ciphertext: MLSCiphertext) -> MLSPlaintext {
        let tree = self.tree.borrow();
        let mut roster = Vec::new();
        for i in 0..tree.leaf_count().as_usize() {
            let node = &tree.nodes[NodeIndex::from(i).as_usize()];
            let credential = if let Some(kp) = &node.key_package {
                kp.get_credential()
            } else {
                panic!("Missing key package");
            };
            roster.push(credential);
        }

        mls_ciphertext.to_plaintext(
            &self.ciphersuite,
            &roster,
            &self.epoch_secrets,
            &mut self.astree.borrow_mut(),
            &self.group_context,
        )
    }

    // Exporter
    fn export_secret(&self, label: &str, key_length: usize) -> Vec<u8> {
        mls_exporter(
            self.get_ciphersuite(),
            &self.epoch_secrets,
            label,
            &self.get_context(),
            key_length,
        )
    }
}

impl Codec for MlsGroup {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        self.ciphersuite.encode(buffer)?;
        self.group_context.encode(buffer)?;
        self.generation.encode(buffer)?;
        self.epoch_secrets.encode(buffer)?;
        self.astree.borrow().encode(buffer)?;
        self.tree.borrow().encode(buffer)?;
        encode_vec(VecSize::VecU8, buffer, &self.interim_transcript_hash)?;
        Ok(())
    }
    fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
        let ciphersuite = Ciphersuite::decode(cursor)?;
        let group_context = GroupContext::decode(cursor)?;
        let generation = u32::decode(cursor)?;
        let epoch_secrets = EpochSecrets::decode(cursor)?;
        let astree = ASTree::decode(cursor)?;
        let tree = RatchetTree::decode(cursor)?;
        let interim_transcript_hash = decode_vec(VecSize::VecU8, cursor)?;
        let group = MlsGroup {
            ciphersuite,
            group_context,
            generation,
            epoch_secrets,
            astree: RefCell::new(astree),
            tree: RefCell::new(tree),
            interim_transcript_hash,
        };
        Ok(group)
    }
}

impl MlsGroup {
    pub fn get_tree(&self) -> Ref<RatchetTree> {
        self.tree.borrow()
    }
    fn get_sender_index(&self) -> LeafIndex {
        self.tree.borrow().get_own_index().into()
    }
    pub(crate) fn get_ciphersuite(&self) -> &Ciphersuite {
        &self.ciphersuite
    }

    pub(crate) fn get_context(&self) -> &GroupContext {
        &self.group_context
    }

    pub(crate) fn get_epoch_secrets(&self) -> &EpochSecrets {
        &self.epoch_secrets
    }
}

// Helper functions

fn update_confirmed_transcript_hash(
    ciphersuite: &Ciphersuite,
    mls_plaintext_commit_content: &MLSPlaintextCommitContent,
    interim_transcript_hash: &[u8],
) -> Vec<u8> {
    let mls_plaintext_commit_content_bytes =
        mls_plaintext_commit_content.encode_detached().unwrap();
    ciphersuite.hash(&[interim_transcript_hash, &mls_plaintext_commit_content_bytes].concat())
}

fn update_interim_transcript_hash(
    ciphersuite: &Ciphersuite,
    mls_plaintext: &MLSPlaintext,
    confirmed_transcript_hash: &[u8],
) -> Vec<u8> {
    let mls_plaintext_auth_data_bytes = &MLSPlaintextCommitAuthData::from(mls_plaintext.clone())
        .encode_detached()
        .unwrap();
    ciphersuite.hash(
        &[
            &confirmed_transcript_hash,
            &mls_plaintext_auth_data_bytes[..],
        ]
        .concat(),
    )
}

fn compute_welcome_key_nonce(
    ciphersuite: &Ciphersuite,
    joiner_secret: &[u8],
) -> (AeadKey, AeadNonce) {
    let welcome_secret = ciphersuite
        .hkdf_expand(joiner_secret, b"mls 1.0 welcome", ciphersuite.hash_length())
        .unwrap();
    let welcome_nonce = AeadNonce::from_slice(
        &ciphersuite
            .hkdf_expand(&welcome_secret, b"nonce", ciphersuite.aead_nonce_length())
            .unwrap(),
    );
    let welcome_key = AeadKey::from_slice(
        &ciphersuite
            .hkdf_expand(&welcome_secret, b"key", ciphersuite.aead_key_length())
            .unwrap(),
    );
    (welcome_key, welcome_nonce)
}
