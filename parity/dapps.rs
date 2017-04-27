// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use std::path::PathBuf;
use std::sync::Arc;

use dir::default_data_path;
use ethcore::client::{Client, BlockChainClient, BlockId};
use ethcore::transaction::{Transaction, Action};
use hash_fetch::fetch::Client as FetchClient;
use hash_fetch::urlhint::ContractClient;
use helpers::replace_home;
use rpc_apis::SignerService;
use parity_reactor;
use util::{Bytes, Address, U256};

#[derive(Debug, PartialEq, Clone)]
pub struct Configuration {
	pub enabled: bool,
	pub dapps_path: PathBuf,
	pub extra_dapps: Vec<PathBuf>,
}

impl Default for Configuration {
	fn default() -> Self {
		let data_dir = default_data_path();
		Configuration {
			enabled: true,
			dapps_path: replace_home(&data_dir, "$BASE/dapps").into(),
			extra_dapps: vec![],
		}
	}
}

impl Configuration {
	pub fn address(&self, address: Option<(String, u16)>) -> Option<(String, u16)> {
		match self.enabled {
			true => address,
			false => None,
		}
	}
}

/// Registrar implementation of the full client.
pub struct FullRegistrar {
	/// Handle to the full client.
	pub client: Arc<Client>,
}

impl ContractClient for FullRegistrar {
	fn registrar(&self) -> Result<Address, String> {
		self.client.additional_params().get("registrar")
			 .ok_or_else(|| "Registrar not defined.".into())
			 .and_then(|registrar| {
				 registrar.parse().map_err(|e| format!("Invalid registrar address: {:?}", e))
			 })
	}

	fn call(&self, address: Address, data: Bytes) -> Result<Bytes, String> {
		let from = Address::default();
		let transaction = Transaction {
			nonce: self.client.latest_nonce(&from),
			action: Action::Call(address),
			gas: U256::from(50_000_000),
			gas_price: U256::default(),
			value: U256::default(),
			data: data,
		}.fake_sign(from);

		self.client.call(&transaction, BlockId::Latest, Default::default())
			.map_err(|e| format!("{:?}", e))
			.map(|executed| {
				executed.output
			})
	}
}

// TODO: light client implementation forwarding to OnDemand and waiting for future
// to resolve.
#[derive(Clone)]
pub struct Dependencies {
	pub sync_status: Arc<SyncStatus>,
	pub contract_client: Arc<ContractClient>,
	pub remote: parity_reactor::TokioRemote,
	pub fetch: FetchClient,
	pub signer: Arc<SignerService>,
	pub ui_address: Option<(String, u16)>,
}

pub fn new(configuration: Configuration, deps: Dependencies) -> Result<Option<Middleware>, String> {
	if !configuration.enabled {
		return Ok(None);
	}

	server::dapps_middleware(
		deps,
		configuration.dapps_path,
		configuration.extra_dapps,
	).map(Some)
}

pub fn new_ui(enabled: bool, deps: Dependencies) -> Result<Option<Middleware>, String> {
	if !enabled {
		return Ok(None);
	}

	server::ui_middleware(
		deps
	).map(Some)
}

pub use self::server::{SyncStatus, Middleware, service};

#[cfg(not(feature = "dapps"))]
mod server {
	use super::Dependencies;
	use std::sync::Arc;
	use std::path::PathBuf;
	use parity_rpc::{hyper, RequestMiddleware, RequestMiddlewareAction};
	use rpc_apis;

	pub type SyncStatus = Fn() -> bool;

	pub struct Middleware;
	impl RequestMiddleware for Middleware {
		fn on_request(
			&self, _req: &hyper::server::Request<hyper::net::HttpStream>, _control: &hyper::Control
		) -> RequestMiddlewareAction {
			unreachable!()
		}
	}

	pub fn dapps_middleware(
		_deps: Dependencies,
		_dapps_path: PathBuf,
		_extra_dapps: Vec<PathBuf>,
	) -> Result<Middleware, String> {
		Err("Your Parity version has been compiled without WebApps support.".into())
	}

	pub fn ui_middleware(
		_deps: Dependencies,
	) -> Result<Middleware, String> {
		Err("Your Parity version has been compiled without UI support.".into())
	}

	pub fn service(_: &Option<Middleware>) -> Option<Arc<rpc_apis::DappsService>> {
		None
	}
}

#[cfg(feature = "dapps")]
mod server {
	use super::Dependencies;
	use std::path::PathBuf;
	use std::sync::Arc;
	use rpc_apis;

	use parity_dapps;
	use parity_reactor;

	pub use parity_dapps::Middleware;
	pub use parity_dapps::SyncStatus;

	pub fn dapps_middleware(
		deps: Dependencies,
		dapps_path: PathBuf,
		extra_dapps: Vec<PathBuf>,
	) -> Result<Middleware, String> {
		let signer = deps.signer;
		let parity_remote = parity_reactor::Remote::new(deps.remote.clone());
		let web_proxy_tokens = Arc::new(move |token| signer.is_valid_web_proxy_access_token(&token));

		Ok(parity_dapps::Middleware::dapps(
			parity_remote,
			deps.ui_address,
			dapps_path,
			extra_dapps,
			deps.contract_client,
			deps.sync_status,
			web_proxy_tokens,
			deps.fetch,
		))
	}

	pub fn ui_middleware(
		deps: Dependencies,
	) -> Result<Middleware, String> {
		let parity_remote = parity_reactor::Remote::new(deps.remote.clone());
		Ok(parity_dapps::Middleware::ui(
			parity_remote,
			deps.contract_client,
			deps.sync_status,
			deps.fetch,
		))
	}

	pub fn service(middleware: &Option<Middleware>) -> Option<Arc<rpc_apis::DappsService>> {
		middleware.as_ref().map(|m| Arc::new(DappsServiceWrapper {
			endpoints: m.endpoints()
		}) as Arc<rpc_apis::DappsService>)
	}

	pub struct DappsServiceWrapper {
		endpoints: parity_dapps::Endpoints,
	}

	impl rpc_apis::DappsService for DappsServiceWrapper {
		fn list_dapps(&self) -> Vec<rpc_apis::LocalDapp> {
			self.endpoints.list()
				.into_iter()
				.map(|app| rpc_apis::LocalDapp {
					id: app.id,
					name: app.name,
					description: app.description,
					version: app.version,
					author: app.author,
					icon_url: app.icon_url,
				})
				.collect()
		}
	}
}
