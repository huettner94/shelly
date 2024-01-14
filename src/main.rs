// Copyright (C) 2024 Felix Huettner
//
// This file is part of shelly-exporter.
//
// shelly-exporter is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// shelly-exporter is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use axum::extract::Query;
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use core::fmt;
use serde_derive::Deserialize;
use serde_json::value::Value;
use std::{collections::HashMap, error::Error, time::Duration};

#[derive(Debug)]
struct ShellyError {
    details: String,
}

impl Error for ShellyError {}

impl fmt::Display for ShellyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

#[derive(Debug, Deserialize)]
struct ProbeParams {
    target: String,
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/probe", get(handler));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

//async fn handler(Query(params): Query<ProbeParams>) -> Result<String, Box<dyn std::error::Error>> {
async fn handler(Query(params): Query<ProbeParams>) -> Result<String, StatusCode> {
    let power = get_power(&params.target).await.map_err(|e| {
        println!("Error in handler {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let mut output = String::new();
    for (key, value) in power {
        output.push_str(&format!(
            "shelly_power_usage_watts{{switch_id=\"{}\"}} {}\n",
            key, value
        ));
    }
    Ok(output)
}

async fn get_power(url: &str) -> Result<HashMap<u32, f64>, Box<dyn std::error::Error>> {
    let resp: serde_json::Value = reqwest::Client::new()
        .get(format!("{}/rpc/Shelly.GetStatus", url))
        .timeout(Duration::from_secs(10))
        .send()
        .await?
        .json()
        .await?;
    let mut result = HashMap::new();

    match resp {
        Value::Object(container) => {
            for (key, value) in container {
                if !key.starts_with("switch:") {
                    continue;
                }
                let switch_id = key[7..].to_string().parse()?;
                let power = value
                    .get("apower")
                    .and_then(|v| v.as_number())
                    .and_then(|v| v.as_f64())
                    .ok_or(Box::new(ShellyError {
                        details: "Did not return valid power reading".to_string(),
                    }))?;
                result.insert(switch_id, power);
            }
        }
        _ => {
            return Err(Box::new(ShellyError {
                details: "Did not return a valid json".to_string(),
            }));
        }
    }

    Ok(result)
}
