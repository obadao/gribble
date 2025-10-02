use std::collections::VecDeque;
use sysinfo::Networks;
use crate::utils::NETWORK_HISTORY_SIZE;

pub struct NetworkHistory {
    pub rx_history: VecDeque<u64>,
    pub tx_history: VecDeque<u64>,
    pub rx_rates: VecDeque<u64>,
    pub tx_rates: VecDeque<u64>,
    pub last_rx_bytes: u64,
    pub last_tx_bytes: u64,
    pub max_history: usize,
    pub current_interface: String,
    pub counter_wrapped: bool,
}

impl NetworkHistory {
    pub fn new() -> Self {
        Self {
            rx_history: VecDeque::with_capacity(NETWORK_HISTORY_SIZE),
            tx_history: VecDeque::with_capacity(NETWORK_HISTORY_SIZE),
            rx_rates: VecDeque::with_capacity(NETWORK_HISTORY_SIZE),
            tx_rates: VecDeque::with_capacity(NETWORK_HISTORY_SIZE),
            last_rx_bytes: 0,
            last_tx_bytes: 0,
            max_history: NETWORK_HISTORY_SIZE,
            current_interface: String::new(),
            counter_wrapped: false,
        }
    }

    pub fn update(&mut self, networks: &Networks, selected_interface: &str) {
        // Find the selected network interface or use the first available one
        let network_list: Vec<_> = networks.list().iter().take(100).collect();
        let (interface_name, network_data) = if let Some(item) = network_list.first() {
            // If we have a specific interface selected, try to find it
            if !selected_interface.is_empty() {
                network_list.iter()
                    .find(|(name, _)| *name == selected_interface)
                    .unwrap_or(item)
            } else {
                item
            }
        } else {
            return; // No network interfaces available
        };

        // Update current interface name
        self.current_interface = interface_name.to_string();

        let total_rx = network_data.total_received();
        let total_tx = network_data.total_transmitted();

        // Detect counter wraparound or interface reset
        if total_rx < self.last_rx_bytes || total_tx < self.last_tx_bytes {
            self.counter_wrapped = true;
            self.last_rx_bytes = total_rx;
            self.last_tx_bytes = total_tx;
            return; // Skip this measurement entirely
        }

        if self.last_rx_bytes > 0 && self.last_tx_bytes > 0 && !self.counter_wrapped {
            let rx_rate = total_rx.saturating_sub(self.last_rx_bytes);
            let tx_rate = total_tx.saturating_sub(self.last_tx_bytes);
            
            self.rx_rates.push_back(rx_rate);
            self.tx_rates.push_back(tx_rate);
            
            if self.rx_rates.len() > self.max_history {
                self.rx_rates.pop_front();
            }
            if self.tx_rates.len() > self.max_history {
                self.tx_rates.pop_front();
            }
        }

        self.rx_history.push_back(total_rx);
        self.tx_history.push_back(total_tx);
        
        if self.rx_history.len() > self.max_history {
            self.rx_history.pop_front();
        }
        if self.tx_history.len() > self.max_history {
            self.tx_history.pop_front();
        }

        self.last_rx_bytes = total_rx;
        self.last_tx_bytes = total_tx;
        self.counter_wrapped = false;
    }

    pub fn clear(&mut self) {
        self.rx_history.clear();
        self.tx_history.clear();
        self.rx_rates.clear();
        self.tx_rates.clear();
        self.last_rx_bytes = 0;
        self.last_tx_bytes = 0;
        self.counter_wrapped = false;
    }
}
