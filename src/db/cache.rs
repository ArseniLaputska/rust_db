// src/db/cache.rs

use lru::LruCache;
use std::sync::{Arc, Mutex};
use std::num::NonZeroUsize;
use uuid::Uuid;

/// Тип кэша для записей контактов (можно аналогично сделать для сообщений)
pub type ContactCache = LruCache<Uuid, super::contact::Contact>;

/// Структура для управления кэшем (можно расширить, если понадобится многоуровневое кэширование)
#[derive(Clone)]
pub struct CacheHandler {
    pub contact_cache: Arc<Mutex<ContactCache>>,
}

impl CacheHandler {
    /// Создаёт новый кэш с заданной ёмкостью
    pub fn new(capacity: usize) -> Self {
        Self {
            contact_cache: Arc::new(Mutex::new(
                LruCache::new(NonZeroUsize::new(capacity).expect("capacity must be nonzero"))
            )),
        }
    }

    /// Пытается получить контакт по UUID из кэша
    pub fn get_contact(&self, id: &Uuid) -> Option<super::contact::Contact> {
        let mut cache = self.contact_cache.lock().unwrap();
        cache.get(id).cloned()
    }

    /// Добавляет или обновляет запись контакта в кэше
    pub fn put_contact(&self, id: Uuid, contact: super::contact::Contact) {
        let mut cache = self.contact_cache.lock().unwrap();
        cache.put(id, contact);
    }
}
