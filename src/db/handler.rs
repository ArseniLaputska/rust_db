// src/db/contact.rs

use tokio_rusqlite::{Connection, Result as SqlResult, Transaction, params};
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::str::FromStr;
use log::{debug, error, info, warn, trace};
use thiserror::Error;
use objc2_foundation::{NSData, NSString};
use objc2::rc::Retained;
use async_trait::async_trait;

#[async_trait]
pub trait EntityRepository<T> {
    async fn get(&self, id: Uuid) -> Result<Option<T>, String>;
    async fn set(&self, entity: T) -> Result<(), String>;
    async fn delete(&self, id: Uuid) -> Result<(), String>;
}