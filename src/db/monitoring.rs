// src/db/monitoring.rs

use std::time::Instant;
use log::{info, warn, error, debug};
use once_cell::sync::Lazy;
use prometheus::{Encoder, TextEncoder, IntCounterVec, HistogramVec, register_int_counter_vec, register_histogram_vec};

/// Глобальные метрики для отслеживания операций с базой данных
pub static DB_QUERY_COUNTER: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "db_query_total",
        "Total number of DB queries executed",
        &["operation"]
    ).expect("Failed to create DB_QUERY_COUNTER")
});

pub static DB_QUERY_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "db_query_duration_seconds",
        "Duration of DB queries in seconds",
        &["operation"],
        vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
    ).expect("Failed to create DB_QUERY_DURATION")
});

/// Функция-обёртка для выполнения операции с базой и сбора метрик.
pub async fn measure_db_operation<F, T>(operation: &str, f: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: std::future::Future<Output = Result<T, Box<dyn std::error::Error>>>,
{
    let start = Instant::now();
    let result = f.await;
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();

    DB_QUERY_COUNTER.with_label_values(&[operation]).inc();
    DB_QUERY_DURATION.with_label_values(&[operation]).observe(secs);

    debug!("DB operation {} took {:.4} seconds", operation, secs);
    result
}

/// Пример использования обёртки внутри репозитория
/*
impl ContactRepo {
    pub async fn get(&self, id: Uuid) -> rusqlite::Result<Option<ContactObjC>> {
        measure_db_operation("get_contact", async {
            // ... Ваш существующий код запроса из БД
        }).await.map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))
    }
}
*/

/// Функция для экспорта метрик в текстовом формате (например, для Prometheus)
pub fn gather_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
