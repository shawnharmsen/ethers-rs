#![cfg(not(target_arch = "wasm32"))]

use std::convert::TryFrom;

use async_trait::async_trait;

use ethers_core::{types::*, utils::Ganache};
use ethers_middleware::gas_oracle::{
    EthGasStation, Etherchain, Etherscan, GasCategory, GasOracle, GasOracleError,
    GasOracleMiddleware,
};
use ethers_providers::{Http, Middleware, Provider};
use serial_test::serial;

#[derive(Debug)]
struct FakeGasOracle {
    gas_price: U256,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl GasOracle for FakeGasOracle {
    async fn fetch(&self) -> Result<U256, GasOracleError> {
        Ok(self.gas_price)
    }

    async fn estimate_eip1559_fees(&self) -> Result<(U256, U256), GasOracleError> {
        Err(GasOracleError::Eip1559EstimationNotSupported)
    }
}

#[tokio::test]
async fn using_gas_oracle() {
    let ganache = Ganache::new().spawn();

    let from = ganache.addresses()[0];

    // connect to the network
    let provider = Provider::<Http>::try_from(ganache.endpoint()).unwrap();

    // this is set because ganache now sets 875000000 as the first block's base fee
    let base_fee = 875000000;
    // assign a gas oracle to use
    let gas_oracle = FakeGasOracle { gas_price: (base_fee + 1337).into() };
    let expected_gas_price = gas_oracle.fetch().await.unwrap();

    let provider = GasOracleMiddleware::new(provider, gas_oracle);

    // broadcast a transaction
    let tx = TransactionRequest::new().from(from).to(Address::zero()).value(10000);
    let tx_hash = provider.send_transaction(tx, None).await.unwrap();

    let tx = provider.get_transaction(*tx_hash).await.unwrap().unwrap();
    assert_eq!(tx.gas_price, Some(expected_gas_price));
}

#[tokio::test]
async fn eth_gas_station() {
    // initialize and fetch gas estimates from EthGasStation
    let eth_gas_station_oracle = EthGasStation::default();
    let data = eth_gas_station_oracle.fetch().await;
    assert!(data.is_ok());
}

#[tokio::test]
#[serial]
async fn etherscan() {
    let etherscan_client = ethers_etherscan::Client::new_from_env(Chain::Mainnet).unwrap();

    // initialize and fetch gas estimates from Etherscan
    // since etherscan does not support `fastest` category, we expect an error
    let etherscan_oracle = Etherscan::new(etherscan_client.clone()).category(GasCategory::Fastest);
    let data = etherscan_oracle.fetch().await;
    assert!(data.is_err());

    // but fetching the `standard` gas price should work fine
    let etherscan_oracle = Etherscan::new(etherscan_client).category(GasCategory::SafeLow);

    let data = etherscan_oracle.fetch().await;
    assert!(data.is_ok());
}

#[tokio::test]
async fn etherchain() {
    // initialize and fetch gas estimates from Etherchain
    let etherchain_oracle = Etherchain::default().category(GasCategory::Fast);
    let data = etherchain_oracle.fetch().await;
    assert!(data.is_ok());
}
