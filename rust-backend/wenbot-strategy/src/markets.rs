//! Weather market data types and scanning

use serde::{Deserialize, Serialize};

/// Weather market info from Polymarket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherMarketInfo {
    pub market_id: String,
    pub condition_id: String,
    pub question: String,
    pub slug: String,
    pub city_key: String,
    pub city_name: String,
    pub target_date: String,
    pub metric: String,          // "high" or "low"
    pub direction: String,       // "above", "below", "between"
    pub threshold_f: f64,
    pub range_low: Option<f64>,  // For "between" markets
    pub range_high: Option<f64>,
    pub yes_price: f64,
    pub no_price: f64,
    pub token_id_yes: String,
    pub token_id_no: String,
    pub active: bool,
}

impl WeatherMarketInfo {
    /// Parse market direction from the question text
    pub fn parse_direction(question: &str) -> Option<(&str, &str, f64)> {
        let q = question.to_lowercase();

        // Detect metric: "high temperature" or "low temperature"
        let metric = if q.contains("low temperature") { "low" } else { "high" };

        // "be above Y°F"
        if let Some(idx) = q.find("be above ") {
            let rest = &q[idx + 9..];
            let parts: Vec<&str> = rest.split('°').collect();
            if parts.len() >= 1 {
                if let Ok(temp) = parts[0].trim().parse::<f64>() {
                    return Some(("above", metric, temp));
                }
            }
        }

        // "be below Y°F"
        if let Some(idx) = q.find("be below ") {
            let rest = &q[idx + 9..];
            let parts: Vec<&str> = rest.split('°').collect();
            if parts.len() >= 1 {
                if let Ok(temp) = parts[0].trim().parse::<f64>() {
                    return Some(("below", metric, temp));
                }
            }
        }

        // "be between X°F and Y°F"
        if let Some(idx) = q.find("be between ") {
            let rest = &q[idx + 11..];
            let temps: Vec<f64> = rest
                .split("°f and ")
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if temps.len() >= 2 {
                return Some(("between", metric, temps[0])); // threshold_f = range_low
            }
        }

        None
    }

    /// Check if the market has enough edge potential
    pub fn has_potential(&self, min_edge: f64) -> bool {
        // Markets near 50/50 have the most edge potential
        let price = self.yes_price;
        price > 0.10 && price < 0.90 && (price - 0.5).abs() < (1.0 - min_edge)
    }
}

/// City configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CityConfig {
    pub key: String,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
}

/// Known city configurations
pub const CITY_CONFIGS: &[(&str, &str, f64, f64, &str)] = &[
    ("nyc", "New York", 40.7128, -74.0060, "America/New_York"),
    ("chicago", "Chicago", 41.8781, -87.6298, "America/Chicago"),
    ("miami", "Miami", 25.7617, -80.1918, "America/New_York"),
    ("los_angeles", "Los Angeles", 34.0522, -118.2437, "America/Los_Angeles"),
    ("denver", "Denver", 39.7392, -104.9903, "America/Denver"),
    ("seattle", "Seattle", 47.6062, -122.3321, "America/Los_Angeles"),
    ("atlanta", "Atlanta", 33.7490, -84.3880, "America/New_York"),
    ("dallas", "Dallas", 32.7767, -96.7970, "America/Chicago"),
    ("london", "London", 51.5074, -0.1278, "Europe/London"),
    ("paris", "Paris", 48.8566, 2.3522, "Europe/Paris"),
    ("tokyo", "Tokyo", 35.6762, 139.6503, "Asia/Tokyo"),
    ("seoul", "Seoul", 37.5665, 126.9780, "Asia/Seoul"),
    ("shanghai", "Shanghai", 31.2304, 121.4737, "Asia/Shanghai"),
];

/// Get city config by key
pub fn get_city_config(key: &str) -> Option<CityConfig> {
    CITY_CONFIGS
        .iter()
        .find(|(k, _, _, _, _)| *k == key)
        .map(|(k, name, lat, lon, tz)| CityConfig {
            key: k.to_string(),
            name: name.to_string(),
            latitude: *lat,
            longitude: *lon,
            timezone: tz.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_direction_above() {
        let q = "Will the high temperature in New York be above 75°F on March 28?";
        let result = WeatherMarketInfo::parse_direction(q);
        assert!(result.is_some());
        let (dir, metric, temp) = result.unwrap();
        assert_eq!(dir, "above");
        assert_eq!(metric, "high");
        assert_eq!(temp, 75.0);
    }

    #[test]
    fn test_parse_direction_below() {
        let q = "Will the high temperature in Chicago be below 30°F on March 28?";
        let result = WeatherMarketInfo::parse_direction(q);
        assert!(result.is_some());
        let (dir, _, temp) = result.unwrap();
        assert_eq!(dir, "below");
        assert_eq!(temp, 30.0);
    }

    #[test]
    fn test_get_city_config() {
        let config = get_city_config("nyc");
        assert!(config.is_some());
        let c = config.unwrap();
        assert_eq!(c.name, "New York");
    }

    #[test]
    fn test_unknown_city() {
        assert!(get_city_config("unknown_city").is_none());
    }

    #[test]
    fn test_parse_low_temperature() {
        let q = "Will the low temperature in Chicago be below 20°F on March 28?";
        let result = WeatherMarketInfo::parse_direction(q);
        assert!(result.is_some());
        let (dir, metric, temp) = result.unwrap();
        assert_eq!(dir, "below");
        assert_eq!(metric, "low");
        assert_eq!(temp, 20.0);
    }
}
