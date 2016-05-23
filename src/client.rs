use msgs::enums::CipherSuite;
use session::SessionSecrets;
use suites::{SupportedCipherSuite, DEFAULT_CIPHERSUITES};
use msgs::handshake::{SessionID, CertificatePayload};
use msgs::handshake::{ServerNameRequest, SupportedSignatureAlgorithms};
use msgs::handshake::{ClientExtension, DigitallySignedStruct};
use msgs::deframer::MessageDeframer;
use msgs::message::Message;
use msgs::base::Payload;
use client_hs;
use hash_hs;
use verify;
use handshake::HandshakeError;
use rand;

use std::sync::Arc;
use std::fmt::Debug;
use std::io;
use std::collections::VecDeque;

pub struct ClientConfig {
  /* List of ciphersuites, in preference order. */
  pub ciphersuites: Vec<&'static SupportedCipherSuite>,

  /* Collection of root certificates. */
  pub root_store: verify::RootCertStore
}

impl ClientConfig {
  pub fn default() -> ClientConfig {
    ClientConfig {
      ciphersuites: DEFAULT_CIPHERSUITES.to_vec(),
      root_store: verify::RootCertStore::empty()
    }
  }
}

pub struct ClientHandshakeData {
  pub client_hello: Vec<u8>,
  pub server_cert_chain: CertificatePayload,
  pub ciphersuite: Option<&'static SupportedCipherSuite>,
  pub dns_name: String,
  pub client_random: Vec<u8>,
  pub server_random: Vec<u8>,
  pub server_kx_params: Vec<u8>,
  pub server_kx_sig: Option<DigitallySignedStruct>,
  pub handshake_hash: Option<hash_hs::HandshakeHash>,
  pub secrets: SessionSecrets
}

impl ClientHandshakeData {
  fn new(host_name: &str) -> ClientHandshakeData {
    ClientHandshakeData {
      client_hello: Vec::new(),
      server_cert_chain: Vec::new(),
      ciphersuite: None,
      dns_name: host_name.to_string(),
      client_random: Vec::new(),
      server_random: Vec::new(),
      server_kx_params: Vec::new(),
      server_kx_sig: None,
      handshake_hash: None,
      secrets: SessionSecrets::for_client()
    }
  }

  pub fn generate_client_random(&mut self) {
    rand::fill_random_vec(&mut self.client_random, 32);
  }

  pub fn hash_message(&mut self, m: &Message) {
    self.handshake_hash.as_mut().unwrap().update(m);
  }

  pub fn get_verify_data(&self) -> Payload {
    Payload { body: self.handshake_hash.as_ref().unwrap().get_current_hash().into_boxed_slice() }
  }
}

pub enum ConnState {
  ExpectServerHello,
  ExpectCertificate,
  ExpectServerKX,
  ExpectServerHelloDone,
  ExpectCCS,
  ExpectFinished,
  Traffic
}

pub struct ClientSession {
  pub config: Arc<ClientConfig>,
  pub handshake_data: ClientHandshakeData,
  pub secrets_current: SessionSecrets,
  pub message_deframer: MessageDeframer,
  pub tls_queue: VecDeque<Message>,
  pub state: ConnState
}

impl ClientSession {
  pub fn new(client_config: &Arc<ClientConfig>,
             hostname: &str) -> ClientSession {
    let mut cs = ClientSession {
      config: client_config.clone(),
      handshake_data: ClientHandshakeData::new(hostname),
      secrets_current: SessionSecrets::for_client(),
      message_deframer: MessageDeframer::new(),
      tls_queue: VecDeque::new(),
      state: ConnState::ExpectServerHello
    };

    client_hs::emit_client_hello(&mut cs);
    cs
  }

  pub fn get_cipher_suites(&self) -> Vec<CipherSuite> {
    let mut ret = Vec::new();

    for cs in self.config.ciphersuites.iter() {
      ret.push(cs.suite.clone());
    }

    /* We don't do renegotation at all, in fact. */
    ret.push(CipherSuite::TLS_EMPTY_RENEGOTIATION_INFO_SCSV);

    ret
  }

  pub fn find_cipher_suite(&self, suite: &CipherSuite) -> Option<&'static SupportedCipherSuite> {
    let got = suite.clone();
    for ref scs in &self.config.ciphersuites {
      if scs.suite == got {
        return Some(scs);
      }
    }

    None
  }

  pub fn add_extensions(&self, exts: &mut Vec<ClientExtension>) {
  }

  pub fn wants_read(&self) -> bool {
    true
  }

  pub fn wants_write(&self) -> bool {
    !self.tls_queue.is_empty()
  }

  pub fn process_msg(&mut self, msg: &mut Message) -> Result<(), HandshakeError> {
    msg.decode_payload();

    let handler = self.get_handler();
    let expects = (handler.expect)();
    try!(expects.check_message(msg));
    let new_state = try!((handler.handle)(self, msg));
    self.state = new_state;

    Ok(())
  }

  fn get_handler(&self) -> &'static client_hs::Handler {
    match self.state {
      ConnState::ExpectServerHello => &client_hs::ExpectServerHello,
      ConnState::ExpectCertificate => &client_hs::ExpectCertificate,
      ConnState::ExpectServerKX => &client_hs::ExpectServerKX,
      ConnState::ExpectServerHelloDone => &client_hs::ExpectServerHelloDone,
      _ => &client_hs::InvalidState
    }
  }

  pub fn process_new_packets(&mut self) -> Result<(), HandshakeError> {
    loop {
      match self.message_deframer.frames.pop_front() {
        Some(mut msg) => try!(self.process_msg(&mut msg)),
        None => break
      }
    }

    Ok(())
  }

  pub fn read_tls(&mut self, rd: &mut io::Read) -> io::Result<usize> {
    self.message_deframer.read(rd)
  }

  pub fn write_tls(&mut self, wr: &mut io::Write) -> io::Result<()> {
    let msg_maybe = self.tls_queue.pop_front();
    if msg_maybe.is_none() {
      return Ok(());
    }

    let mut data = Vec::new();
    let msg = msg_maybe.unwrap();
    println!("writing {:?}", msg);
    msg.encode(&mut data);

    println!("write {:?}", data);

    wr.write_all(&data)
  }
}
