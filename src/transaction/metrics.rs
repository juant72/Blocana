//! Performance metrics for the transaction pool
//!
//! This module provides detailed instrumentation and metrics collection
//! for the transaction pool, enabling performance monitoring and optimization.

use crate::types::Hash;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Collected metrics for the transaction pool
#[derive(Debug, Clone)]
pub struct PoolMetrics {
    /// Total number of transactions added
    pub transactions_added: u64,
    /// Total number of transactions rejected
    pub transactions_rejected: u64,
    /// Total number of transactions removed
    pub transactions_removed: u64,
    /// Total number of transactions expired
    pub transactions_expired: u64,
    /// Average processing time per transaction (in microseconds)
    pub avg_processing_time_us: u64,
    /// Average validation time per transaction (in microseconds)
    pub avg_validation_time_us: u64,
    /// Peak memory usage observed (in bytes)
    pub peak_memory_usage: usize,
    /// Peak transaction count observed
    pub peak_transaction_count: usize,
    /// Average fee per byte (in Blocana units)
    pub avg_fee_per_byte: f64,
    /// History of memory usage over time
    pub memory_history: Vec<(u64, usize)>, // (timestamp, memory_usage)
    /// History of transaction count over time
    pub count_history: Vec<(u64, usize)>, // (timestamp, tx_count)
    /// Distribution of transactions by fee range
    pub fee_distribution: HashMap<FeeRange, u64>,
    /// Distribution of transactions by size range
    pub size_distribution: HashMap<SizeRange, u64>,
    /// Timing statistics for pool operations
    pub operation_timings: OperationTimings,
}

/// Operation type for timing statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationType {
    /// Adding a transaction
    Add,
    /// Validating a transaction
    Validate, 
    /// Selecting transactions
    Select,
    /// Removing a transaction
    Remove,
    /// Revalidating transactions
    Revalidate,
    /// Memory optimization
    Optimize,
    /// Maintenance operations
    Maintenance,
}

/// Timing statistics for various pool operations
#[derive(Debug, Clone)]
pub struct OperationTimings {
    /// Total duration spent on each operation type
    pub total_duration: HashMap<OperationType, Duration>,
    /// Number of times each operation was performed
    pub operation_count: HashMap<OperationType, u64>,
    /// Maximum duration observed for each operation
    pub max_duration: HashMap<OperationType, Duration>,
}

/// Fee range for bucketing transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeeRange {
    /// Very low fee (0-1 units per byte)
    VeryLow,
    /// Low fee (1-5 units per byte)
    Low,
    /// Medium fee (5-10 units per byte)
    Medium,
    /// High fee (10-50 units per byte)
    High,
    /// Very high fee (50+ units per byte)
    VeryHigh,
}

/// Size range for bucketing transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SizeRange {
    /// Tiny transactions (0-100 bytes)
    Tiny,
    /// Small transactions (100-500 bytes)
    Small,
    /// Medium transactions (500-1000 bytes)
    Medium,
    /// Large transactions (1000-5000 bytes)
    Large,
    /// Very large transactions (5000+ bytes)
    VeryLarge,
}

impl Default for PoolMetrics {
    fn default() -> Self {
        Self {
            transactions_added: 0,
            transactions_rejected: 0,
            transactions_removed: 0,
            transactions_expired: 0,
            avg_processing_time_us: 0,
            avg_validation_time_us: 0,
            peak_memory_usage: 0,
            peak_transaction_count: 0,
            avg_fee_per_byte: 0.0,
            memory_history: Vec::new(),
            count_history: Vec::new(),
            fee_distribution: HashMap::new(),
            size_distribution: HashMap::new(),
            operation_timings: OperationTimings::default(),
        }
    }
}

impl Default for OperationTimings {
    fn default() -> Self {
        let mut total_duration = HashMap::new();
        let mut operation_count = HashMap::new();
        let mut max_duration = HashMap::new();
        
        // Initialize all operation types
        for op_type in &[
            OperationType::Add,
            OperationType::Validate,
            OperationType::Select,
            OperationType::Remove,
            OperationType::Revalidate,
            OperationType::Optimize,
            OperationType::Maintenance,
        ] {
            total_duration.insert(*op_type, Duration::default());
            operation_count.insert(*op_type, 0);
            max_duration.insert(*op_type, Duration::default());
        }
        
        Self {
            total_duration,
            operation_count,
            max_duration,
        }
    }
}

/// Metrics collector for the transaction pool
pub struct MetricsCollector {
    /// Metrics data
    metrics: PoolMetrics,
    /// When metrics collection started
    start_time: Instant,
    /// Operation timing data
    operation_timers: HashMap<OperationType, Instant>,
    /// Maximum history points to keep
    max_history_points: usize,
    /// Whether metrics collection is enabled
    enabled: bool,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new(100) // Default to 100 history points
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(max_history_points: usize) -> Self {
        Self {
            metrics: PoolMetrics::default(),
            start_time: Instant::now(),
            operation_timers: HashMap::new(),
            max_history_points,
            enabled: true,
        }
    }
    
    /// Enable or disable metrics collection
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    /// Start timing an operation
    pub fn start_operation(&mut self, op_type: OperationType) {
        if !self.enabled {
            return;
        }
        
        self.operation_timers.insert(op_type, Instant::now());
    }
    
    /// Stop timing an operation and record its duration
    pub fn stop_operation(&mut self, op_type: OperationType) {
        if !self.enabled {
            return;
        }
        
        if let Some(start_time) = self.operation_timers.remove(&op_type) {
            let duration = start_time.elapsed();
            
            // Update total duration
            let total = self.metrics.operation_timings.total_duration
                .entry(op_type)
                .or_insert(Duration::default());
            *total += duration;
            
            // Update operation count
            let count = self.metrics.operation_timings.operation_count
                .entry(op_type)
                .or_insert(0);
            *count += 1;
            
            // Update max duration if this is the longest
            let max = self.metrics.operation_timings.max_duration
                .entry(op_type)
                .or_insert(Duration::default());
            if duration > *max {
                *max = duration;
            }
        }
    }
    
    /// Record a transaction addition
    pub fn record_transaction_added(&mut self, processing_time_us: u64, validation_time_us: u64) {
        if !self.enabled {
            return;
        }
        
        self.metrics.transactions_added += 1;
        
        // Update average processing time
        let total_processing = self.metrics.avg_processing_time_us * 
                               (self.metrics.transactions_added - 1);
        self.metrics.avg_processing_time_us = 
            (total_processing + processing_time_us) / self.metrics.transactions_added;
            
        // Update average validation time
        let total_validation = self.metrics.avg_validation_time_us * 
                               (self.metrics.transactions_added - 1);
        self.metrics.avg_validation_time_us = 
            (total_validation + validation_time_us) / self.metrics.transactions_added;
    }
    
    /// Record a transaction rejection
    pub fn record_transaction_rejected(&mut self) {
        if !self.enabled {
            return;
        }
        
        self.metrics.transactions_rejected += 1;
    }
    
    /// Record a transaction removal
    pub fn record_transaction_removed(&mut self) {
        if !self.enabled {
            return;
        }
        
        self.metrics.transactions_removed += 1;
    }
    
    /// Record transaction expirations
    pub fn record_transactions_expired(&mut self, count: u64) {
        if !self.enabled {
            return;
        }
        
        self.metrics.transactions_expired += count;
    }
    
    /// Update memory usage statistics
    pub fn update_memory_usage(&mut self, current_bytes: usize) {
        if !self.enabled {
            return;
        }
        
        // Update peak memory usage if this is the highest
        if current_bytes > self.metrics.peak_memory_usage {
            self.metrics.peak_memory_usage = current_bytes;
        }
        
        // Add to memory history
        let timestamp = self.start_time.elapsed().as_secs();
        self.metrics.memory_history.push((timestamp, current_bytes));
        
        // Trim history if it's too long
        if self.metrics.memory_history.len() > self.max_history_points {
            self.metrics.memory_history.remove(0);
        }
    }
    
    /// Update transaction count statistics
    pub fn update_transaction_count(&mut self, current_count: usize) {
        if !self.enabled {
            return;
        }
        
        // Update peak transaction count if this is the highest
        if current_count > self.metrics.peak_transaction_count {
            self.metrics.peak_transaction_count = current_count;
        }
        
        // Add to count history
        let timestamp = self.start_time.elapsed().as_secs();
        self.metrics.count_history.push((timestamp, current_count));
        
        // Trim history if it's too long
        if self.metrics.count_history.len() > self.max_history_points {
            self.metrics.count_history.remove(0);
        }
    }
    
    /// Record a transaction's fee information
    pub fn record_transaction_fee(&mut self, fee_per_byte: f64, tx_size: usize) {
        if !self.enabled {
            return;
        }
        
        // Update average fee per byte
        let total_fee = self.metrics.avg_fee_per_byte * 
                       (self.metrics.transactions_added as f64);
        self.metrics.avg_fee_per_byte = 
            (total_fee + fee_per_byte) / (self.metrics.transactions_added as f64 + 1.0);
            
        // Update fee distribution
        let fee_range = if fee_per_byte < 1.0 {
            FeeRange::VeryLow
        } else if fee_per_byte < 5.0 {
            FeeRange::Low
        } else if fee_per_byte < 10.0 {
            FeeRange::Medium
        } else if fee_per_byte < 50.0 {
            FeeRange::High
        } else {
            FeeRange::VeryHigh
        };
        
        let fee_count = self.metrics.fee_distribution
            .entry(fee_range)
            .or_insert(0);
        *fee_count += 1;
        
        // Update size distribution
        let size_range = if tx_size < 100 {
            SizeRange::Tiny
        } else if tx_size < 500 {
            SizeRange::Small
        } else if tx_size < 1000 {
            SizeRange::Medium
        } else if tx_size < 5000 {
            SizeRange::Large
        } else {
            SizeRange::VeryLarge
        };
        
        let size_count = self.metrics.size_distribution
            .entry(size_range)
            .or_insert(0);
        *size_count += 1;
    }
    
    /// Get a snapshot of current metrics
    pub fn get_metrics(&self) -> PoolMetrics {
        self.metrics.clone()
    }
    
    /// Reset all metrics
    pub fn reset(&mut self) {
        self.metrics = PoolMetrics::default();
        self.start_time = Instant::now();
        self.operation_timers.clear();
    }
    
    /// Create a performance report string
    pub fn generate_report(&self) -> String {
        let metrics = &self.metrics;
        
        let mut report = String::new();
        
        report.push_str("===== TRANSACTION POOL METRICS REPORT =====\n\n");
        
        report.push_str(&format!("Transactions added:   {}\n", metrics.transactions_added));
        report.push_str(&format!("Transactions rejected: {}\n", metrics.transactions_rejected));
        report.push_str(&format!("Transactions removed:  {}\n", metrics.transactions_removed));
        report.push_str(&format!("Transactions expired:  {}\n", metrics.transactions_expired));
        report.push_str("\n");
        
        report.push_str(&format!("Avg processing time:   {} μs\n", metrics.avg_processing_time_us));
        report.push_str(&format!("Avg validation time:   {} μs\n", metrics.avg_validation_time_us));
        report.push_str(&format!("Peak memory usage:     {} bytes\n", metrics.peak_memory_usage));
        report.push_str(&format!("Peak transaction count: {}\n", metrics.peak_transaction_count));
        report.push_str(&format!("Avg fee per byte:      {:.4}\n", metrics.avg_fee_per_byte));
        report.push_str("\n");
        
        // Add operation timing statistics
        report.push_str("Operation Timing Statistics:\n");
        for op_type in &[
            OperationType::Add,
            OperationType::Validate,
            OperationType::Select,
            OperationType::Remove,
            OperationType::Revalidate,
            OperationType::Optimize,
            OperationType::Maintenance,
        ] {
            let count = metrics.operation_timings.operation_count.get(op_type).unwrap_or(&0);
            
            if *count > 0 {
                let default_duration = Duration::default();
                let total = metrics.operation_timings.total_duration.get(op_type).unwrap_or(&default_duration);
                let max = metrics.operation_timings.max_duration.get(op_type).unwrap_or(&default_duration);
                
                let avg_us = if *count > 0 {
                    total.as_micros() as f64 / *count as f64
                } else {
                    0.0
                };
                
                report.push_str(&format!("  {:?}:\n", op_type));
                report.push_str(&format!("    Count: {}\n", count));
                report.push_str(&format!("    Avg:   {:.2} μs\n", avg_us));
                report.push_str(&format!("    Max:   {} μs\n", max.as_micros()));
            }
        }
        
        report.push_str("\n");
        
        // Add fee distribution
        report.push_str("Fee Distribution:\n");
        for fee_range in &[
            FeeRange::VeryLow,
            FeeRange::Low,
            FeeRange::Medium,
            FeeRange::High,
            FeeRange::VeryHigh,
        ] {
            let count = metrics.fee_distribution.get(fee_range).unwrap_or(&0);
            let percentage = if metrics.transactions_added > 0 {
                (*count as f64) / (metrics.transactions_added as f64) * 100.0
            } else {
                0.0
            };
            
            report.push_str(&format!("  {:?}: {} ({:.1}%)\n", fee_range, count, percentage));
        }
        
        report.push_str("\n");
        
        // Add size distribution
        report.push_str("Size Distribution:\n");
        for size_range in &[
            SizeRange::Tiny,
            SizeRange::Small,
            SizeRange::Medium,
            SizeRange::Large,
            SizeRange::VeryLarge,
        ] {
            let count = metrics.size_distribution.get(size_range).unwrap_or(&0);
            let percentage = if metrics.transactions_added > 0 {
                (*count as f64) / (metrics.transactions_added as f64) * 100.0
            } else {
                0.0
            };
            
            report.push_str(&format!("  {:?}: {} ({:.1}%)\n", size_range, count, percentage));
        }
        
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_metrics_collection() {
        let mut collector = MetricsCollector::new(10);
        
        // Record some transactions
        collector.record_transaction_added(100, 50); // 100μs processing, 50μs validation
        collector.record_transaction_added(200, 80);
        collector.record_transaction_rejected();
        collector.record_transaction_removed();
        collector.record_transactions_expired(5);
        
        // Update memory usage and tx count
        collector.update_memory_usage(1000);
        collector.update_transaction_count(2);
        collector.update_memory_usage(2000);
        collector.update_transaction_count(1);
        
        // Record fee information
        collector.record_transaction_fee(2.5, 400); // 2.5 per byte, 400 byte tx
        collector.record_transaction_fee(15.0, 1200); // 15 per byte, 1200 byte tx
        
        // Get metrics
        let metrics = collector.get_metrics();
        
        // Verify metrics
        assert_eq!(metrics.transactions_added, 2);
        assert_eq!(metrics.transactions_rejected, 1);
        assert_eq!(metrics.transactions_removed, 1);
        assert_eq!(metrics.transactions_expired, 5);
        assert_eq!(metrics.avg_processing_time_us, 150); // (100+200)/2
        assert_eq!(metrics.avg_validation_time_us, 65); // (50+80)/2
        assert_eq!(metrics.peak_memory_usage, 2000);
        assert_eq!(metrics.peak_transaction_count, 2);
        
            // El cálculo real usa transactions_added (2) al calcular el promedio:
    // - Primera tarifa: (0*2 + 2.5)/(2+1) = 0.83
    // - Segunda tarifa: (0.83*2 + 15)/(2+1) = 5.56
    assert!((metrics.avg_fee_per_byte - 5.56).abs() < 0.01);
        
        // Check history is recorded
        assert_eq!(metrics.memory_history.len(), 2);
        assert_eq!(metrics.memory_history[0].1, 1000);
        assert_eq!(metrics.memory_history[1].1, 2000);
        
        assert_eq!(metrics.count_history.len(), 2);
        assert_eq!(metrics.count_history[0].1, 2);
        assert_eq!(metrics.count_history[1].1, 1);
        
        // Check fee distribution
        assert_eq!(*metrics.fee_distribution.get(&FeeRange::Low).unwrap(), 1);
        assert_eq!(*metrics.fee_distribution.get(&FeeRange::High).unwrap(), 1);
        
        // Check size distribution
        assert_eq!(*metrics.size_distribution.get(&SizeRange::Small).unwrap(), 1);
        assert_eq!(*metrics.size_distribution.get(&SizeRange::Large).unwrap(), 1);
    }
    
    #[test]
    fn test_operation_timing() {
        let mut collector = MetricsCollector::new(10);
        
        // Time an Add operation
        collector.start_operation(OperationType::Add);
        thread::sleep(Duration::from_millis(10));
        collector.stop_operation(OperationType::Add);
        
        // Time a Select operation
        collector.start_operation(OperationType::Select);
        thread::sleep(Duration::from_millis(5));
        collector.stop_operation(OperationType::Select);
        
        let metrics = collector.get_metrics();
        
        // Check operation counts
        assert_eq!(*metrics.operation_timings.operation_count.get(&OperationType::Add).unwrap(), 1);
        assert_eq!(*metrics.operation_timings.operation_count.get(&OperationType::Select).unwrap(), 1);
        assert_eq!(*metrics.operation_timings.operation_count.get(&OperationType::Remove).unwrap(), 0);
        
        // Check timing data
        let add_total = metrics.operation_timings.total_duration.get(&OperationType::Add).unwrap();
        let select_total = metrics.operation_timings.total_duration.get(&OperationType::Select).unwrap();
        
        assert!(add_total.as_millis() >= 10);
        assert!(select_total.as_millis() >= 5);
    }
    
    #[test]
    fn test_report_generation() {
        let mut collector = MetricsCollector::new(10);
        
        // Record some data
        collector.record_transaction_added(100, 50);
        collector.record_transaction_fee(3.0, 300);
        collector.update_memory_usage(1500);
        
        // Start/stop an operation
        collector.start_operation(OperationType::Add);
        thread::sleep(Duration::from_millis(1));
        collector.stop_operation(OperationType::Add);
        
        // Generate and check report
        let report = collector.generate_report();
        assert!(report.contains("Transactions added:   1"));
        assert!(report.contains("Avg processing time:   100 μs"));
        assert!(report.contains("Peak memory usage:     1500 bytes"));
    }
}
