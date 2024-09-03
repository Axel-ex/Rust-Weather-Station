### About

The **Rust Weather Station** is a fully-featured weather monitoring system built using Rust and embedded hardware components. Designed to be reliable, this project leverages the power of Rust for embedded systems to gather real-time environmental data such as temperature, humidity, pressure, wind speed, wind direction, and rainfall. The gathered data is then published to an MQTT server, making it accessible for further use.

The weather station integrates several sensors using I2C protocol and handles data communication over Wi-Fi. This project showcases how Rust can be utilized for low-level hardware interaction, real-time data processing, and IoT applications.

<img src="demo.jpg" width="50%" />

### Hardware

- **Microcontroller**: ESP32.
- **Sensors**:
  - **AS5600**: A magnetic rotary position sensor for detecting wind direction.
  - **BME680**: An environmental sensor for measuring temperature, humidity, pressure, and gas.
  - **Hall effect sensors**: to embed into the anemometer and rain gauge.
  - **18650 Lithium ion battery**.
  - **12V solar pannel**: charges the batteries.
  - **CN3791 solar charger**: charges the batteries when the sun is up. Protects against overcharge and overdischarge.
  - **MT3608 step up converter**: Steps up the output voltage to 5v to feed the esp32.

### Key Features

- **Multiple Sensors**: Supports various sensors for comprehensive weather data collection.
- **MQTT Integration**: Data is published to an MQTT broker, making it easy to integrate with IoT platforms like Home Assistant.
- **Interrupt Handling**: Uses GPIO interrupts to handle events such as wind speed changes and rainfall detection.
- **Light sleep mode**: the esp32 is configured to enter light sleep mode as much as he can to save up battery power.

### Implementation

- **Sensor Integration**:
  - **AS5600**: The AS5600 sensor is used to measure wind direction. It communicates via I2C, and the data is read and processed to determine the exact direction of the wind.
  - **BME680**: This sensor provides temperature, humidity, pressure, and gas readings. It is also connected via I2C and configured with custom settings to ensure accurate environmental data collection.
  Since Rust borrow checker doesn't allow sharing multiple mutable references, *embedded_hal_bus* crate was used since it provides utilities to share the I2C driver between the peripherals.

- **Interrupts Handling**:
  - **Anemometer and Rain Gauge**: These sensors use GPIO pins to generate interrupts based on the triggering of hall effect sensor by the passage of a magnet above. Upon the trigerring of an interrupt, the esp wakes up from light sleep and check which pins received the interrupt. Because of the API design, interrupt have to be manually reactivated outside of the ISR upon fireing. Everytime some interrupts are detected, global counts storing the rain falling and the rotation count of the anemo are incremented to later be publish.

- **MQTT Communication**:
  - **Data Publishing**: The collected data from the sensors is published to an MQTT broker using the MQTT protocol. The `publish_wifi_data`, `publish_bme_data`, `publish_anemo_data`, and `publish_rain_data` functions handle the publication of different sensor data. Data is published at regular interval allowing the esp to enter light sleep mode when innactive.
  - **Connection Management**: The MQTT connection is maintained in a separate thread, ensuring that the weather station remains connected to the broker and can send/receive messages in real-time.


### Resources
All .stl files can be downloaded from this link (https://www.printables.com/model/729382-yaws-yet-another-weather-station/files) ready to be printed!
