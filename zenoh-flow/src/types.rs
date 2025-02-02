//
// Copyright (c) 2017, 2021 ADLINK Technology Inc.
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ADLINK zenoh team, <zenoh@adlink-labs.tech>
//

use crate::async_std::sync::{Arc, Mutex, MutexGuard};
use crate::runtime::message::{ZFControlMessage, ZFDataMessage, ZFMessage};
use crate::serde::{Deserialize, Serialize};
use futures::Future;
use std::any::Any;
use std::collections::HashMap;
use std::convert::From;
use std::fmt::Debug;
use std::pin::Pin;
use uhlc::Timestamp;

// Placeholder types
pub type ZFOperatorId = String;
pub type ZFZenohResource = String;
pub type ZFOperatorName = String;
pub type ZFTimestamp = usize; //TODO: improve it, usize is just a placeholder

pub type ZFPortID = String;
pub type ZFRuntimeID = String;

#[derive(Debug, PartialEq)]
pub enum ZFError {
    GenericError,
    SerializationError,
    DeseralizationError,
    MissingState,
    InvalidState,
    Unimplemented,
    Empty,
    MissingConfiguration,
    VersionMismatch,
    Disconnected,
    Uncompleted(String),
    PortTypeNotMatching((String, String)),
    OperatorNotFound(ZFOperatorId),
    PortNotFound((ZFOperatorId, String)),
    RecvError(flume::RecvError),
    SendError(String),
    MissingInput(String),
    MissingOutput(String),
    InvalidData(String),
    IOError(String),
    ZenohError(String),
    LoadingError(String),
    ParsingError(String),
}

impl From<flume::RecvError> for ZFError {
    fn from(err: flume::RecvError) -> Self {
        Self::RecvError(err)
    }
}

impl From<flume::TryRecvError> for ZFError {
    fn from(err: flume::TryRecvError) -> Self {
        match err {
            flume::TryRecvError::Disconnected => Self::Disconnected,
            flume::TryRecvError::Empty => Self::Empty,
        }
    }
}

impl<T> From<flume::SendError<T>> for ZFError {
    fn from(err: flume::SendError<T>) -> Self {
        Self::SendError(format!("{:?}", err))
    }
}

impl From<std::io::Error> for ZFError {
    fn from(err: std::io::Error) -> Self {
        Self::IOError(format!("{}", err))
    }
}

impl From<zenoh_util::core::ZError> for ZFError {
    fn from(err: zenoh_util::core::ZError) -> Self {
        Self::ZenohError(format!("{}", err))
    }
}

impl From<libloading::Error> for ZFError {
    fn from(err: libloading::Error) -> Self {
        Self::LoadingError(format!("Error when loading the library: {}", err))
    }
}

pub struct ZFInnerCtx {
    pub state: Box<dyn StateTrait>,
    pub mode: usize, //can be arc<atomic> and inside ZFContext
}

#[derive(Clone)]
pub struct ZFContext(Arc<Mutex<ZFInnerCtx>>); //TODO: have only state inside Mutex

impl ZFContext {
    pub fn new(state: Box<dyn StateTrait>, mode: usize) -> Self {
        let inner = Arc::new(Mutex::new(ZFInnerCtx { state, mode }));
        Self(inner)
    }

    pub async fn async_lock(&'_ self) -> MutexGuard<'_, ZFInnerCtx> {
        self.0.lock().await
    }

    pub fn lock(&self) -> MutexGuard<ZFInnerCtx> {
        crate::zf_spin_lock!(self.0)
    }
}

pub type ZFResult<T> = Result<T, ZFError>;

#[typetag::serde(tag = "zf_data_type", content = "value")]
pub trait DataTrait: Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_mut_any(&mut self) -> &mut dyn Any;
}

// TODO Create a Derive macro for this
#[typetag::serde(tag = "zf_state_type", content = "value")]
pub trait StateTrait: Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_mut_any(&mut self) -> &mut dyn Any;
}

pub trait OperatorMode: Into<usize> + From<usize> {} //Placeholder

#[derive(Debug, Clone)]
pub enum ZFComponentOutput {
    Data(Arc<dyn DataTrait>),
    // TODO Users should not have access to all control messages. When implementing the control
    // messages change this to an enum with a "limited scope".
    Control(ZFControlMessage),
}

pub type InputRuleOutput = ZFResult<bool>;

// CAUTION, USER CAN DO NASTY THINGS, eg. remove a link we have passed to him.
pub type FnInputRule =
    dyn Fn(ZFContext, &mut HashMap<String, Token>) -> InputRuleOutput + Send + Sync + 'static;

pub type OutputRuleOutput = ZFResult<HashMap<ZFPortID, ZFComponentOutput>>;

pub type FnOutputRule = dyn Fn(ZFContext, HashMap<ZFPortID, Arc<dyn DataTrait>>) -> OutputRuleOutput
    + Send
    + Sync
    + 'static;

pub type RunOutput = ZFResult<HashMap<ZFPortID, Arc<dyn DataTrait>>>;

pub type FnRun = dyn Fn(ZFContext, ZFInput) -> RunOutput + Send + Sync + 'static;

// * ***********************************************************************************************
// * *
// * * OPERATOR
// * *
// * ***********************************************************************************************
pub trait OperatorTrait {
    fn get_input_rule(&self, ctx: ZFContext) -> Box<FnInputRule>;

    fn get_output_rule(&self, ctx: ZFContext) -> Box<FnOutputRule>;

    fn get_run(&self, ctx: ZFContext) -> Box<FnRun>;

    fn get_state(&self) -> Box<dyn StateTrait>;
}

// * ***********************************************************************************************
// *
// *   SOURCE
// *
// * ***********************************************************************************************
pub type FutRunOutput = Pin<Box<dyn Future<Output = RunOutput> + Send + Sync>>;

pub type FnSourceRun = Box<dyn Fn(ZFContext) -> FutRunOutput + Send + Sync>;

pub trait SourceTrait {
    fn get_run(&self, ctx: ZFContext) -> FnSourceRun;

    fn get_output_rule(&self, ctx: ZFContext) -> Box<FnOutputRule>;

    fn get_state(&self) -> Box<dyn StateTrait>;
}

// * ***********************************************************************************************
// *
// * SINK
// *
// * ***********************************************************************************************
pub type FutSinkOutput = Pin<Box<dyn Future<Output = ZFResult<()>> + Send + Sync>>;

pub type FnSinkRun = Box<dyn Fn(ZFContext, ZFInput) -> FutSinkOutput + Send + Sync>;

pub trait SinkTrait {
    fn get_input_rule(&self, ctx: ZFContext) -> Box<FnInputRule>;

    fn get_run(&self, ctx: ZFContext) -> FnSinkRun;

    fn get_state(&self) -> Box<dyn StateTrait>;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TokenAction {
    Consume, // Default, data is passed to the run and consumed from the "thing"
    Drop,    // Data is dropped
    KeepRun, // Data is passed to the run and kept in the "thing"
    Keep,    // Data is kept in the "thing"
    Wait,    //Waits the Data, this is applicable only to NotReadyToken
}

#[derive(Debug, Clone, Default)]
pub struct NotReadyToken;

#[derive(Debug, Clone)]
pub struct ReadyToken {
    pub data: ZFDataMessage,
    pub action: TokenAction,
}

#[derive(Debug, Clone)]
pub enum Token {
    NotReady,
    Ready(ReadyToken),
}

impl Token {
    pub fn new_ready(data: ZFDataMessage) -> Self {
        Self::Ready(ReadyToken {
            data,
            action: TokenAction::Consume,
        })
    }

    pub fn get_timestamp(&self) -> Option<Timestamp> {
        match self {
            Self::NotReady => None,
            Self::Ready(token) => Some(token.data.timestamp.clone()),
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    pub fn is_not_ready(&self) -> bool {
        matches!(self, Self::NotReady)
    }

    pub fn consume(&mut self) -> ZFResult<()> {
        match self {
            Self::Ready(ref mut ready) => {
                ready.action = TokenAction::Consume;
                Ok(())
            }
            _ => Err(ZFError::GenericError),
        }
    }

    pub fn drop(&mut self) -> ZFResult<()> {
        match self {
            Self::Ready(ref mut ready) => {
                ready.action = TokenAction::Drop;
                Ok(())
            }
            _ => Err(ZFError::GenericError),
        }
    }

    pub fn keep_run(&mut self) -> ZFResult<()> {
        match self {
            Self::Ready(ref mut ready) => {
                ready.action = TokenAction::KeepRun;
                Ok(())
            }
            _ => Err(ZFError::GenericError),
        }
    }

    pub fn keep(&mut self) -> ZFResult<()> {
        match self {
            Self::Ready(ref mut ready) => {
                ready.action = TokenAction::Keep;
                Ok(())
            }
            _ => Err(ZFError::GenericError),
        }
    }

    pub fn data(&self) -> ZFResult<ZFDataMessage> {
        match self {
            Self::Ready(ready) => Ok(ready.data.clone()),
            _ => Err(ZFError::GenericError),
        }
    }

    pub fn action(&self) -> &TokenAction {
        match self {
            Self::Ready(ready) => &ready.action,
            Self::NotReady => &TokenAction::Wait,
        }
    }

    pub fn split(self) -> (Option<ZFDataMessage>, TokenAction) {
        match self {
            Self::Ready(ready) => (Some(ready.data), ready.action),
            Self::NotReady => (None, TokenAction::Wait),
        }
    }
}

impl From<Arc<ZFMessage>> for Token {
    fn from(message: Arc<ZFMessage>) -> Self {
        match message.as_ref() {
            ZFMessage::Control(_) => Token::NotReady,
            ZFMessage::Data(data_message) => Token::new_ready(data_message.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZFInput(HashMap<String, ZFDataMessage>);

impl Default for ZFInput {
    fn default() -> Self {
        Self::new()
    }
}

impl ZFInput {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn insert(&mut self, id: String, data: ZFDataMessage) -> Option<ZFDataMessage> {
        self.0.insert(id, data)
    }

    pub fn get(&self, id: &str) -> Option<&ZFDataMessage> {
        self.0.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ZFDataMessage> {
        self.0.get_mut(id)
    }
}

impl<'a> IntoIterator for &'a ZFInput {
    type Item = (&'a String, &'a ZFDataMessage);
    type IntoIter = std::collections::hash_map::Iter<'a, String, ZFDataMessage>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmptyState;

#[typetag::serde]
impl StateTrait for EmptyState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

pub fn default_output_rule(
    _ctx: ZFContext,
    outputs: HashMap<String, Arc<dyn DataTrait>>,
) -> OutputRuleOutput {
    let mut results = HashMap::new();
    for (k, v) in outputs {
        results.insert(k, ZFComponentOutput::Data(v));
    }

    Ok(results)
}

pub fn default_input_rule(_ctx: ZFContext, inputs: &mut HashMap<String, Token>) -> InputRuleOutput {
    for token in inputs.values() {
        match token {
            Token::Ready(_) => continue,
            Token::NotReady => return Ok(false),
        }
    }

    Ok(true)
}
