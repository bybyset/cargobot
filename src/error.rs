use std::ffi::NulError;
use esp_idf_svc::sys::EspError;


pub enum BotError {
    StorageError,
    EspError(EspError),
    CStringError(NulError),
}