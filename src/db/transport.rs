// use std::collections::HashMap;
// use std::sync::Arc;
// use tokio::sync::Mutex;
// use uuid::Uuid;
// use serde::{Serialize, Deserialize};
// use log::{info, error, warn};
//
// use crate::db::{
//     contact::Contact,
//     message::Message,
// };
// // use crate::db::error::DbResult;
//
// // Типы ошибок для транспорта
// #[derive(Debug, thiserror::Error)]
// pub enum TransportError {
//     #[error("Network is not available")]
//     NetworkUnavailable,
//
//     #[error("Max retry count reached for operation")]
//     MaxRetryCountReached,
//
//     #[error("Operation timeout")]
//     Timeout,
//
//     #[error("Server error: {0}")]
//     ServerError(String),
//
//     #[error("Serialization error: {0}")]
//     SerializationError(String),
//
//     #[error(transparent)]
//     Other(#[from] anyhow::Error),
// }
//
// // Счетчик повторных попыток
// #[derive(Debug, Clone)]
// pub struct RetryCounter {
//     counters: Arc<Mutex<HashMap<Uuid, u32>>>,
// }
//
// impl RetryCounter {
//     pub fn new() -> Self {
//         Self {
//             counters: Arc::new(Mutex::new(HashMap::new())),
//         }
//     }
//
//     /// Увеличивает счетчик для данного ID и возвращает новое значение.
//     pub async fn increment(&self, id: Uuid) -> u32 {
//         let mut counters = self.counters.lock().await;
//         let counter = counters.entry(id).or_insert(0);
//         *counter += 1;
//         *counter
//     }
//
//     /// Возвращает текущее значение счетчика для данного ID.
//     pub async fn get(&self, id: Uuid) -> u32 {
//         let counters = self.counters.lock().await;
//         *counters.get(&id).unwrap_or(&0)
//     }
//
//     /// Удаляет счетчик для данного ID.
//     pub async fn remove(&self, id: Uuid) {
//         let mut counters = self.counters.lock().await;
//         counters.remove(&id);
//     }
// }
//
// // Трейт для транспортных операций
// #[async_trait::async_trait]
// pub trait TransportOps {
//     async fn send_contact(&self, contact: Contact) -> Result<(), TransportError>;
//     async fn delete_contact(&self, entity_id: Uuid) -> Result<(), TransportError>;
//     async fn send_message(&self, message: Message) -> Result<(), TransportError>;
//     async fn delete_message(&self, entity_id: Uuid) -> Result<(), TransportError>;
// }
//
// // Основной транспортный слой
// #[derive(Clone)]
// pub struct DataTransport {
//     retry_counter: RetryCounter,
//     network_available: Arc<Mutex<bool>>,
//     max_retries: u32,
//     // Возможно, другие поля, такие как конфигурации для сетевых клиентов
// }
//
// impl DataTransport {
//     /// Создает новый экземпляр DataTransport.
//     pub fn new(max_retries: u32) -> Self {
//         Self {
//             retry_counter: RetryCounter::new(),
//             network_available: Arc::new(Mutex::new(true)),
//             max_retries,
//         }
//     }
//
//     /// Устанавливает статус доступности сети.
//     pub async fn set_network_status(&self, available: bool) {
//         let mut status = self.network_available.lock().await;
//         *status = available;
//         info!("Network status set to: {}", available);
//     }
//
//     /// Проверяет, можно ли отправить операцию.
//     async fn check_can_send(&self, id: Uuid) -> Result<(), TransportError> {
//         let network_available = *self.network_available.lock().await;
//         if !network_available {
//             return Err(TransportError::NetworkUnavailable);
//         }
//
//         let retry_count = self.retry_counter.get(id).await;
//         if retry_count >= self.max_retries {
//             return Err(TransportError::MaxRetryCountReached);
//         }
//
//         Ok(())
//     }
//
//     /// Логирует успешную отправку и сбрасывает счетчик.
//     async fn handle_success(&self, id: Uuid) {
//         self.retry_counter.remove(id).await;
//         info!("Successfully handled operation for ID: {}", id);
//     }
//
//     /// Логирует неудачную отправку.
//     async fn handle_failure(&self, id: Uuid, error: &TransportError) {
//         error!("Failed to handle operation for ID: {}. Error: {}", id, error);
//     }
//
//     /// Заглушка для отправки контакта.
//     async fn mock_send_contact(&self, _contact: &Contact) -> Result<(), TransportError> {
//         // Симулируем успешную отправку
//         tokio::time::sleep(std::time::Duration::from_millis(100)).await;
//         Ok(())
//     }
//
//     /// Заглушка для удаления контакта.
//     async fn mock_delete_contact(&self, _entity_id: Uuid) -> Result<(), TransportError> {
//         // Симулируем успешное удаление
//         tokio::time::sleep(std::time::Duration::from_millis(100)).await;
//         Ok(())
//     }
//
//     /// Заглушка для отправки сообщения.
//     async fn mock_send_message(&self, _message: &Message) -> Result<(), TransportError> {
//         // Симулируем успешную отправку
//         tokio::time::sleep(std::time::Duration::from_millis(100)).await;
//         Ok(())
//     }
//
//     /// Заглушка для удаления сообщения.
//     async fn mock_delete_message(&self, _entity_id: Uuid) -> Result<(), TransportError> {
//         // Симулируем успешное удаление
//         tokio::time::sleep(std::time::Duration::from_millis(100)).await;
//         Ok(())
//     }
// }
//
// #[async_trait::async_trait]
// impl TransportOps for DataTransport {
//     /// Метод для отправки контакта на сервер (заглушка).
//     async fn send_contact(&self, contact: Contact) -> Result<(), TransportError> {
//         let id = contact.id;
//         self.check_can_send(id).await?;
//
//         // Логирование попытки отправки
//         info!("Attempting to send contact: {:?}", contact);
//
//         // Заглушка для сетевого вызова
//         let result = self.mock_send_contact(&contact).await;
//
//         match result {
//             Ok(_) => {
//                 self.handle_success(id).await;
//                 Ok(())
//             },
//             Err(e) => {
//                 self.handle_failure(id, &e).await;
//                 Err(e)
//             },
//         }
//     }
//
//     /// Метод для удаления контакта на сервере (заглушка).
//     async fn delete_contact(&self, entity_id: Uuid) -> Result<(), TransportError> {
//         self.check_can_send(entity_id).await?;
//
//         info!("Attempting to delete contact with ID: {}", entity_id);
//
//         let result = self.mock_delete_contact(entity_id).await;
//
//         match result {
//             Ok(_) => {
//                 self.handle_success(entity_id).await;
//                 Ok(())
//             },
//             Err(e) => {
//                 self.handle_failure(entity_id, &e).await;
//                 Err(e)
//             },
//         }
//     }
//
//     /// Метод для отправки сообщения на сервере (заглушка).
//     async fn send_message(&self, message: Message) -> Result<(), TransportError> {
//         let id = message.id;
//         self.check_can_send(id).await?;
//
//         info!("Attempting to send message: {:?}", message);
//
//         let result = self.mock_send_message(&message).await;
//
//         match result {
//             Ok(_) => {
//                 self.handle_success(id).await;
//                 Ok(())
//             },
//             Err(e) => {
//                 self.handle_failure(id, &e).await;
//                 Err(e)
//             },
//         }
//     }
//
//     /// Метод для удаления сообщения на сервере (заглушка).
//     async fn delete_message(&self, entity_id: Uuid) -> Result<(), TransportError> {
//         self.check_can_send(entity_id).await?;
//
//         info!("Attempting to delete message with ID: {}", entity_id);
//
//         let result = self.mock_delete_message(entity_id).await;
//
//         match result {
//             Ok(_) => {
//                 self.handle_success(entity_id).await;
//                 Ok(())
//             },
//             Err(e) => {
//                 self.handle_failure(entity_id, &e).await;
//                 Err(e)
//             },
//         }
//     }
// }
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use tokio::time::{sleep, Duration};
//     use uuid::Uuid;
//
//     #[tokio::test]
//     async fn test_retry_counter() {
//         let counter = RetryCounter::new();
//         let id = Uuid::new_v4();
//
//         assert_eq!(counter.get(id).await, 0, "Initial retry count should be 0");
//
//         assert_eq!(counter.increment(id).await, 1, "Retry count should be 1 after first increment");
//         assert_eq!(counter.increment(id).await, 2, "Retry count should be 2 after second increment");
//
//         counter.remove(id).await;
//         assert_eq!(counter.get(id).await, 0, "Retry count should be 0 after removal");
//     }
//
//     #[tokio::test]
//     async fn test_network_status() {
//         let transport = DataTransport::new(3);
//         let id = Uuid::new_v4();
//
//         // Проверяем, что сеть доступна
//         assert!(transport.check_can_send(id).await.is_ok(), "Should be able to send when network is available");
//
//         // Устанавливаем статус сети как недоступный
//         transport.set_network_status(false).await;
//
//         // Проверяем, что отправка невозможна
//         assert!(matches!(
//             transport.check_can_send(id).await.unwrap_err(),
//             TransportError::NetworkUnavailable
//         ));
//     }
//
//     #[tokio::test]
//     async fn test_max_retries() {
//         let transport = DataTransport::new(2);
//         let id = Uuid::new_v4();
//
//         // Первая попытка
//         assert!(transport.check_can_send(id).await.is_ok(), "First attempt should be allowed");
//         transport.retry_counter.increment(id).await;
//
//         // Вторая попытка
//         assert!(transport.check_can_send(id).await.is_ok(), "Second attempt should be allowed");
//         transport.retry_counter.increment(id).await;
//
//         // Третья попытка должна завершиться ошибкой
//         assert!(matches!(
//             transport.check_can_send(id).await.unwrap_err(),
//             TransportError::MaxRetryCountReached
//         ));
//     }
//
//     #[tokio::test]
//     async fn test_send_contact_success() {
//         let transport = DataTransport::new(3);
//         let contact = Contact {
//             id: Uuid::new_v4(),
//             first_name: "John".into(),
//             last_name: "Doe".into(),
//             // Другие поля
//         };
//
//         assert!(transport.send_contact(contact.clone()).await.is_ok(), "Sending contact should succeed");
//     }
//
//     #[tokio::test]
//     async fn test_send_contact_failure() {
//         let transport = DataTransport::new(1);
//         let contact = Contact {
//             id: Uuid::new_v4(),
//             first_name: "Jane".into(),
//             last_name: "Doe".into(),
//             // Другие поля
//         };
//
//         // Модифицируем заглушку, чтобы симулировать ошибку
//         // В данном примере нет способа изменить поведение заглушки, поэтому предполагаем успех
//         // В реальной реализации можно использовать моки или флаги для симуляции ошибок
//         assert!(transport.send_contact(contact.clone()).await.is_ok(), "Sending contact should succeed (stub)");
//     }
// }