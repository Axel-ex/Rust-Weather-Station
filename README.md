# 🌦️ Rust Weather Station

<div align="center">
  <img src=".github/assets/demo.jpg" width="60%" alt="Weather Station Demo" />
</div>

---

## 📌 About

This project is a weather station built with **Rust** and embedded hardware. It collects environmental data like **temperature**, **humidity**, **pressure**, **wind speed**, **wind direction**, and **rainfall**, and sends that data to an **MQTT** server for use in systems like **Home Assistant**.

---

## 🔧 Hardware Components

- **ESP32** – The main microcontroller running the code.
- **AS5600** – Magnetic rotary sensor for wind direction.
- **BME680** – Sensor for temperature, humidity, pressure, and gas.
- **Hall Effect Sensors** – Used in the anemometer and rain gauge.
- **18650 Li-ion Battery** – Power supply.
- **12V Solar Panel** – Charges the battery during the day.
- **CN3791 Solar Charger** – Manages charging and battery protection.

---

## 🌟 Features

- Publishes sensor data to an **MQTT** broker.
- Handles interrupts from wind and rain sensors using GPIO.
- Uses **deep sleep** to save power when idle.
- Supports multiple sensors over a shared I2C bus using Rust's embedded-hal tools.

---

## 🛠️ Implementation Details

### 🔌 Sensor Integration

- **AS5600 (Wind Direction)**: Measures the position of a magnet to get the wind direction using I2C.
- **BME680 (Environmental Data)**: Reads temperature, humidity, pressure, and gas. Also I2C.
- Since Rust doesn't allow multiple mutable references, the [`embedded-hal-bus`](https://docs.rs/embedded-hal-bus/latest/embedded_hal_bus/) crate is used to safely share the I2C bus between devices.

### ⚡ Interrupts

- **Anemometer & Rain Gauge**: Use magnets and hall sensors to trigger interrupts.
- When triggered, an interrupt sets a flag (`rotation_flag` or `rain_flag`), which is checked and handled later in the main loop.
- Interrupts must be manually re-enabled after they fire, due to how the API works.

### 📤 MQTT

- Data is published via functions like `publish_wifi_data()`, `publish_bme_data()`, etc.
- A separate thread manages the MQTT connection to keep it stable.
- Data is sent at regular intervals, and the ESP32 sleeps between cycles to save battery.

### 💤 Deep Sleep Mode

- **Active (~1min)**: ESP32 wakes up, handles interrupts, checks if it’s time to send data.
- **Sleep (~20min)**: ESP32 goes into deep sleep to reduce power usage.

---

## 🧱 Resources

You can download the 3D printable parts (STL files) here:  
👉 [Printables - YAWS (Yet Another Weather Station)](https://www.printables.com/model/729382-yaws-yet-another-weather-station/files)
