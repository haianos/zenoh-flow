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

use crate::model::period::ZFPeriodDescriptor;
use async_std::sync::Arc;
use std::time::Duration;
use uhlc::{Timestamp, HLC, NTP64};

/// A `PeriodicHLC` is a wrapper around a `HLC` that can generate periodic timestamps.
///
/// A "periodic" timestamp is a timestamp that is a multiple of a period. If no period is
/// associated to the `PeriodicHLC`, the method `new_timestamp` returns the Timestamp generated by
/// the wrapped `HLC`.
pub struct PeriodicHLC {
    hlc: Arc<HLC>,
    period: Option<Duration>,
}

impl PeriodicHLC {
    pub fn new(hlc: Arc<HLC>, period: Option<ZFPeriodDescriptor>) -> Self {
        match period {
            Some(period_descriptor) => Self {
                hlc,
                period: Some(period_descriptor.to_duration()),
            },
            None => Self { hlc, period: None },
        }
    }

    pub fn new_timestamp(&self) -> Timestamp {
        let mut timestamp = self.hlc.new_timestamp();
        log::debug!("Timestamp generated: {:?}", timestamp);

        if let Some(period) = &self.period {
            let period_us = period.as_secs_f64();
            let orig_timestamp_us = timestamp.get_time().to_duration().as_secs_f64();

            let nb_period_floored = f64::floor(orig_timestamp_us / period_us);
            let periodic_timestamp_us = Duration::from_secs_f64(period_us * nb_period_floored);

            timestamp = Timestamp::new(
                NTP64::from(periodic_timestamp_us),
                timestamp.get_id().to_owned(),
            );
            log::debug!(
                "Periodic timestamp: {:?} — period = {:?} — original = {:?}",
                periodic_timestamp_us,
                period_us,
                orig_timestamp_us,
            );
        }

        timestamp
    }
}
