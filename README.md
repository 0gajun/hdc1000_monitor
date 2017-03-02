# HDC1000\_MONITOR (Writen in Rust)

This program monitors temperature and humidity using [HDC1000](http://akizukidenshi.com/catalog/g/gM-08775/) and send monitored data to InfluxDB.

# Notice
This program is created for using on RaspberryPi3.
So I have to do cross compiling. Default build target is specified at `.cargo/config`.

# ScreenShot
If you use grafana and InfluxDB, you can create a dashboard like this.


![Dashboard](https://raw.githubusercontent.com/0gajun/hdc1000_monitor/master/doc/screenshot.png)¬
