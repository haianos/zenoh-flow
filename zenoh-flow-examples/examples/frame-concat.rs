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

use async_std::sync::{Arc, Mutex};
use std::collections::HashMap;
use zenoh_flow::{
    downcast, get_input,
    serde::{Deserialize, Serialize},
    types::{
        DataTrait, FnInputRule, FnOutputRule, FnRun, OperatorTrait, RunOutput, StateTrait,
        ZFContext, ZFInput, ZFResult,
    },
    zenoh_flow_derive::ZFState,
    zf_data, zf_spin_lock,
};
use zenoh_flow_examples::ZFBytes;

use opencv::core;

static INPUT1: &str = "Frame1";
static INPUT2: &str = "Frame2";
static OUTPUT: &str = "Frame";

#[derive(Debug)]
struct FrameConcat {
    pub state: ConcatState,
}

#[derive(Clone)]
struct ConcatInnerState {
    pub encode_options: Arc<Mutex<opencv::types::VectorOfi32>>,
}

#[derive(Serialize, Deserialize, ZFState, Clone)]
struct ConcatState {
    #[serde(skip_serializing, skip_deserializing)]
    pub inner: Option<ConcatInnerState>,
}

// because of opencv
impl std::fmt::Debug for ConcatState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ConcatState:...",)
    }
}

impl FrameConcat {
    fn new(_configuration: HashMap<String, String>) -> Self {
        let encode_options = opencv::types::VectorOfi32::new();

        let inner = Some(ConcatInnerState {
            encode_options: Arc::new(Mutex::new(encode_options)),
        });
        let state = ConcatState { inner };

        Self { state }
    }

    pub fn run_1(ctx: ZFContext, mut inputs: ZFInput) -> RunOutput {
        let mut results: HashMap<String, Arc<dyn DataTrait>> = HashMap::new();

        let guard = ctx.lock(); //getting state
        let _state = downcast!(ConcatState, guard.state).unwrap(); //downcasting to right type

        let inner = _state.inner.as_ref().unwrap();

        let encode_options = zf_spin_lock!(inner.encode_options);

        let (_, frame1) = get_input!(ZFBytes, String::from(INPUT1), inputs)?;
        let (_, frame2) = get_input!(ZFBytes, String::from(INPUT2), inputs)?;

        // Decode Image
        let frame1 = opencv::imgcodecs::imdecode(
            &opencv::types::VectorOfu8::from_iter(frame1.bytes),
            opencv::imgcodecs::IMREAD_COLOR,
        )
        .unwrap();

        let frame2 = opencv::imgcodecs::imdecode(
            &opencv::types::VectorOfu8::from_iter(frame2.bytes),
            opencv::imgcodecs::IMREAD_COLOR,
        )
        .unwrap();

        let mut frame = core::Mat::default();

        //concat frames
        core::vconcat2(&frame1, &frame2, &mut frame).unwrap();

        let mut buf = opencv::types::VectorOfu8::new();
        opencv::imgcodecs::imencode(".jpg", &frame, &mut buf, &encode_options).unwrap();

        let data = ZFBytes {
            bytes: buf.to_vec(),
        };

        results.insert(String::from(OUTPUT), zf_data!(data));

        Ok(results)
    }
}

impl OperatorTrait for FrameConcat {
    fn get_input_rule(&self, _ctx: ZFContext) -> Box<FnInputRule> {
        Box::new(zenoh_flow::default_input_rule)
    }

    fn get_output_rule(&self, _ctx: ZFContext) -> Box<FnOutputRule> {
        Box::new(zenoh_flow::default_output_rule)
    }

    fn get_run(&self, _ctx: ZFContext) -> Box<FnRun> {
        Box::new(Self::run_1)
    }

    fn get_state(&self) -> Box<dyn StateTrait> {
        Box::new(self.state.clone())
    }
}

// //Also generated by macro
zenoh_flow::export_operator!(register);

extern "C" fn register(
    configuration: Option<HashMap<String, String>>,
) -> ZFResult<Box<dyn zenoh_flow::OperatorTrait + Send>> {
    match configuration {
        Some(config) => {
            Ok(Box::new(FrameConcat::new(config)) as Box<dyn zenoh_flow::OperatorTrait + Send>)
        }
        None => {
            Ok(Box::new(FrameConcat::new(HashMap::new()))
                as Box<dyn zenoh_flow::OperatorTrait + Send>)
        }
    }
}
