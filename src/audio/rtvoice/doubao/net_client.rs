
pub struct NetClient {
    pub url: String,
    pub session_id: String,
    pub connected: bool,
}


pub struct NetClientError {
    pub code: i32,
    pub message: String,
}

impl NetClient {
    pub fn new(url: String) -> Self {

        Self {
            url,
            session_id: String::new(),
            connected: false,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn check_connected(&self) -> Result<(), NetClientError> {
        if self.connected {
            return Ok(());
        }
        Err(NetClientError {
            code: ERROR_CODE_INIT_FAILURE,
            message: "NetClient 未连接".to_string(),
        })
    }


    pub async fn connect(&mut self) -> Result<(), NetClientError> {
        if self.connected {
            return Err(NetClientError {
                code: ERROR_CODE_INIT_FAILURE,
                message: "NetClient 已连接".to_string(),
            });
        }


        Ok(())
        
    }

    pub fn send_audio(&mut self, audio_data: &[i16]) {
        
    }


    pub fn close(&mut self) {
        
    }
}



