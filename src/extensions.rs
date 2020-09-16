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

use crate::ciphersuite::*;
use crate::codec::*;
use crate::tree::node::*;
use crate::utils::*;
use std::cmp::Ordering;
use std::mem;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum ProtocolVersion {
    Mls10 = 0,
    Default = 255,
}

impl From<u8> for ProtocolVersion {
    fn from(a: u8) -> ProtocolVersion {
        unsafe { mem::transmute(a) }
    }
}

impl PartialOrd for ProtocolVersion {
    fn partial_cmp(&self, other: &ProtocolVersion) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProtocolVersion {
    fn cmp(&self, other: &ProtocolVersion) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl Codec for ProtocolVersion {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        (*self as u8).encode(buffer)?;
        Ok(())
    }

    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let version = u8::decode(cursor)?;
    //     Ok(version.into())
    // }
}

pub const CURRENT_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::Mls10;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ExtensionType {
    Invalid = 0,
    Capabilities = 1,
    Lifetime = 2,
    KeyID = 3,
    ParentHash = 4,
    RatchetTree = 5,
    DeviceCapabilities = 6,
    Default = 65535,
}

impl From<u16> for ExtensionType {
    fn from(a: u16) -> ExtensionType {
        unsafe { mem::transmute(a) }
    }
}

impl Codec for ExtensionType {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        (*self as u16).encode(buffer)?;
        Ok(())
    }

    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let extension = u16::decode(cursor)?;
    //     Ok(extension.into())
    // }
}

#[derive(PartialEq, Clone, Debug)]
pub enum ExtensionPayload {
    Capabilities(CapabilitiesExtension),
    Lifetime(LifetimeExtension),
    KeyID(KeyIDExtension),
    ParentHash(ParentHashExtension),
    RatchetTree(RatchetTreeExtension),
}

#[derive(PartialEq, Clone, Debug)]
pub struct CapabilitiesExtension {
    pub versions: Vec<ProtocolVersion>,
    pub ciphersuites: Vec<CiphersuiteName>,
    pub extensions: Vec<ExtensionType>,
}

impl CapabilitiesExtension {
    pub fn new(
        versions: Vec<ProtocolVersion>,
        ciphersuites: Vec<CiphersuiteName>,
        extensions: Vec<ExtensionType>,
    ) -> Self {
        CapabilitiesExtension {
            versions,
            ciphersuites,
            extensions,
        }
    }
    pub fn new_from_bytes(bytes: &[u8]) -> Self {
        let cursor = &mut Cursor::new(bytes);
        let versions = decode_vec(VecSize::VecU8, cursor).unwrap();
        let ciphersuites = decode_vec(VecSize::VecU8, cursor).unwrap();
        let extensions = decode_vec(VecSize::VecU8, cursor).unwrap();
        CapabilitiesExtension {
            versions,
            ciphersuites,
            extensions,
        }
    }
    pub fn to_extension(&self) -> Extension {
        let mut extension_data: Vec<u8> = vec![];
        encode_vec(VecSize::VecU8, &mut extension_data, &self.versions).unwrap();
        encode_vec(VecSize::VecU8, &mut extension_data, &self.ciphersuites).unwrap();
        encode_vec(VecSize::VecU8, &mut extension_data, &self.extensions).unwrap();
        let extension_type = ExtensionType::Capabilities;
        Extension {
            extension_type,
            extension_data,
        }
    }
}
#[derive(PartialEq, Clone, Debug)]
pub struct LifetimeExtension {
    not_before: u64,
    not_after: u64,
}

impl LifetimeExtension {
    pub const LIFETIME_1_MINUTE: u64 = 60;
    pub const LIFETIME_1_HOUR: u64 = 60 * LifetimeExtension::LIFETIME_1_MINUTE;
    pub const LIFETIME_1_DAY: u64 = 24 * LifetimeExtension::LIFETIME_1_HOUR;
    pub const LIFETIME_1_WEEK: u64 = 7 * LifetimeExtension::LIFETIME_1_DAY;
    pub const LIFETIME_4_WEEKS: u64 = 4 * LifetimeExtension::LIFETIME_1_WEEK;
    pub const LIFETIME_MARGIN: u64 = LifetimeExtension::LIFETIME_1_HOUR;
    pub fn new(t: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let not_before = now - LifetimeExtension::LIFETIME_MARGIN;
        let not_after = now + t + LifetimeExtension::LIFETIME_MARGIN;
        Self {
            not_before,
            not_after,
        }
    }
    pub fn new_from_bytes(bytes: &[u8]) -> Self {
        let mut cursor = Cursor::new(bytes);
        let not_before = u64::decode(&mut cursor).unwrap();
        let not_after = u64::decode(&mut cursor).unwrap();
        Self {
            not_before,
            not_after,
        }
    }
    pub fn to_extension(&self) -> Extension {
        let mut extension_data: Vec<u8> = vec![];
        self.not_before.encode(&mut extension_data).unwrap();
        self.not_after.encode(&mut extension_data).unwrap();
        let extension_type = ExtensionType::Lifetime;
        Extension {
            extension_type,
            extension_data,
        }
    }
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.not_before < now && self.not_after > now
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct KeyIDExtension {
    key_id: Vec<u8>,
}

impl KeyIDExtension {
    pub fn new_from_bytes(bytes: &[u8]) -> Self {
        let cursor = &mut Cursor::new(bytes);
        let key_id = decode_vec(VecSize::VecU16, cursor).unwrap();
        Self { key_id }
    }
    pub fn to_extension(&self) -> Extension {
        let mut extension_data: Vec<u8> = vec![];
        encode_vec(VecSize::VecU16, &mut extension_data, &self.key_id).unwrap();
        let extension_type = ExtensionType::KeyID;
        Extension {
            extension_type,
            extension_data,
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct ParentHashExtension {
    pub parent_hash: Vec<u8>,
}

impl ParentHashExtension {
    pub fn new(hash: &[u8]) -> Self {
        ParentHashExtension {
            parent_hash: hash.to_vec(),
        }
    }
    pub fn new_from_bytes(bytes: &[u8]) -> Self {
        let cursor = &mut Cursor::new(bytes);
        let parent_hash = decode_vec(VecSize::VecU8, cursor).unwrap();
        Self { parent_hash }
    }
    pub fn to_extension(&self) -> Extension {
        let mut extension_data: Vec<u8> = vec![];
        encode_vec(VecSize::VecU8, &mut extension_data, &self.parent_hash).unwrap();
        let extension_type = ExtensionType::ParentHash;
        Extension {
            extension_type,
            extension_data,
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct RatchetTreeExtension {
    pub tree: Vec<Option<Node>>,
}

impl RatchetTreeExtension {
    pub fn new(tree: Vec<Option<Node>>) -> Self {
        RatchetTreeExtension { tree }
    }
    pub fn new_from_bytes(bytes: &[u8]) -> Self {
        let cursor = &mut Cursor::new(bytes);
        let tree = decode_vec(VecSize::VecU32, cursor).unwrap();
        Self { tree }
    }
    pub fn to_extension(&self) -> Extension {
        let mut extension_data: Vec<u8> = vec![];
        encode_vec(VecSize::VecU32, &mut extension_data, &self.tree).unwrap();
        let extension_type = ExtensionType::RatchetTree;
        Extension {
            extension_type,
            extension_data,
        }
    }
}

#[repr(u32)]
#[cfg_attr(rustfmt, rustfmt_skip)]
pub enum DeviceCapabilityType {
    AddCap          = 0b0001u32,
    RemoveCap       = 0b0010u32,
    NonRemovableCap = 0b0100u32,
    SelfUpdateCap   = 0b1000u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceCapabilities(pub u32);

impl DeviceCapabilities {
    pub fn is_permanent(&self) -> bool {
        self.0 & DeviceType::PermanentDevice as u32 == DeviceType::PermanentDevice as u32
    }
    pub fn is_temporary(&self) -> bool {
        self.0 == DeviceType::TemporaryDevice as u32
    }
    pub fn can_add(&self) -> bool {
        (self.0 & DeviceCapabilityType::AddCap as u32) > 0
    }
    pub fn can_remove(&self) -> bool {
        (self.0 & DeviceCapabilityType::RemoveCap as u32) > 0
    }
    pub fn cannot_be_removed(&self) -> bool {
        (self.0 & DeviceCapabilityType::NonRemovableCap as u32) > 0
    }
    pub fn can_self_update(&self) -> bool {
        (self.0 & DeviceCapabilityType::SelfUpdateCap as u32) > 0
    }
}

#[repr(u32)]
pub enum DeviceType {
    TemporaryDevice = 0u32,
    PermanentDevice = DeviceCapabilityType::AddCap as u32
        | DeviceCapabilityType::RemoveCap as u32
        | DeviceCapabilityType::SelfUpdateCap as u32,
}

impl From<DeviceType> for DeviceCapabilities {
    fn from(n: DeviceType) -> DeviceCapabilities {
        DeviceCapabilities(n as u32)
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct DeviceCapabilityExtension {
    pub capabilities: DeviceCapabilities,
}

impl DeviceCapabilityExtension {
    pub fn new(capabilities: DeviceCapabilities) -> Self {
        DeviceCapabilityExtension { capabilities }
    }
    pub fn new_from_bytes(bytes: &[u8]) -> Self {
        let cursor = &mut Cursor::new(bytes);
        let capabilities = DeviceCapabilities(Codec::decode(cursor).unwrap());
        Self { capabilities }
    }
    pub fn to_extension(&self) -> Extension {
        let mut extension_data: Vec<u8> = vec![];
        self.capabilities.0.encode(&mut extension_data).unwrap();
        let extension_type = ExtensionType::DeviceCapabilities;
        Extension {
            extension_type,
            extension_data,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Extension {
    pub extension_type: ExtensionType,
    pub extension_data: Vec<u8>,
}

impl Extension {
    pub fn get_type(&self) -> ExtensionType {
        self.extension_type
    }
}

impl Codec for Extension {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        self.extension_type.encode(buffer)?;
        encode_vec(VecSize::VecU16, buffer, &self.extension_data)?;
        Ok(())
    }

    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let extension_type = ExtensionType::decode(cursor)?;
    //     let extension_data = decode_vec(VecSize::VecU16, cursor)?;
    //     Ok(Extension {
    //         extension_type,
    //         extension_data,
    //     })
    // }
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct KeyPackageId {
    uuid: Uuid,
}

impl KeyPackageId {
    pub fn new() -> Self {
        let uuid = Uuid::from_slice(&randombytes(16)).unwrap();
        Self { uuid }
    }
    pub fn from_slice(bytes: &[u8]) -> Self {
        let uuid = Uuid::from_slice(bytes).unwrap();
        Self { uuid }
    }
    pub fn to_vec(&self) -> Vec<u8> {
        let bytes = self.uuid.as_bytes();
        bytes.to_vec()
    }
}

impl Codec for KeyPackageId {
    fn encode(&self, buffer: &mut Vec<u8>) -> Result<(), CodecError> {
        encode_vec(VecSize::VecU8, buffer, &self.to_vec())?;
        Ok(())
    }

    // fn decode(cursor: &mut Cursor) -> Result<Self, CodecError> {
    //     let bytes = decode_vec(VecSize::VecU8, cursor)?;
    //     let id = KeyPackageId::from_slice(&bytes);
    //     Ok(id)
    // }
}

#[test]
fn test_protocol_version() {
    let mls10_version = ProtocolVersion::Mls10;
    let default_version = ProtocolVersion::Default;
    let mls10_e = mls10_version.encode_detached().unwrap();
    assert_eq!(mls10_e[0], mls10_version as u8);
    let default_e = default_version.encode_detached().unwrap();
    assert_eq!(default_e[0], default_version as u8);
    assert_eq!(mls10_e[0], 0);
    assert_eq!(default_e[0], 255);
}

#[test]
fn test_extension_codec() {
    use crate::key_packages::*;

    let capabilities_extension = CapabilitiesExtension::new(
        SUPPORTED_PROTOCOL_VERSIONS.to_vec(),
        CIPHERSUITES.to_vec(),
        SUPPORTED_EXTENSIONS.to_vec(),
    );
    let extension = capabilities_extension.to_extension();
    let bytes = extension.encode_detached().unwrap();
    // let _dec = Extension::decode(&mut Cursor::new(&bytes));
}
