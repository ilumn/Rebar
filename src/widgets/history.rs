use crate::system::SystemSnapshot;

#[derive(Debug, Clone)]
pub(crate) struct MetricSeries {
    capacity: usize,
    values: Vec<f32>,
}

impl MetricSeries {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            values: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn push(&mut self, value: f32) {
        if self.values.len() == self.capacity {
            self.values.remove(0);
        }

        self.values.push(value);
    }

    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }

    pub(crate) fn max(&self) -> f32 {
        self.values
            .iter()
            .copied()
            .fold(0.0_f32, |left, right| left.max(right))
    }

    pub(crate) fn line_points(&self) -> Vec<[f64; 2]> {
        self.values
            .iter()
            .enumerate()
            .map(|(index, value)| [index as f64, *value as f64])
            .collect()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WidgetHistory {
    pub(crate) cpu_usage: MetricSeries,
    pub(crate) memory_usage: MetricSeries,
    pub(crate) gpu_memory_usage: MetricSeries,
    pub(crate) network_down_mbps: MetricSeries,
    pub(crate) network_up_mbps: MetricSeries,
}

impl Default for WidgetHistory {
    fn default() -> Self {
        Self::new(300)
    }
}

impl WidgetHistory {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            cpu_usage: MetricSeries::new(capacity),
            memory_usage: MetricSeries::new(capacity),
            gpu_memory_usage: MetricSeries::new(capacity),
            network_down_mbps: MetricSeries::new(capacity),
            network_up_mbps: MetricSeries::new(capacity),
        }
    }

    pub(crate) fn observe(&mut self, snapshot: &SystemSnapshot) {
        let memory_usage = if snapshot.memory.total_bytes == 0 {
            0.0
        } else {
            snapshot.memory.used_bytes as f32 / snapshot.memory.total_bytes as f32 * 100.0
        };

        let gpu_memory_usage = snapshot
            .gpus
            .iter()
            .filter(|gpu| gpu.local_budget_bytes > 0)
            .map(|gpu| gpu.local_usage_bytes as f32 / gpu.local_budget_bytes as f32 * 100.0)
            .fold(0.0_f32, f32::max);

        self.cpu_usage.push(snapshot.cpu.usage_percent.clamp(0.0, 100.0));
        self.memory_usage.push(memory_usage.clamp(0.0, 100.0));
        self.gpu_memory_usage
            .push(gpu_memory_usage.clamp(0.0, 100.0));
        self.network_down_mbps
            .push(snapshot.network.received_bps as f32 / 1_048_576.0);
        self.network_up_mbps
            .push(snapshot.network.transmitted_bps as f32 / 1_048_576.0);
    }
}
