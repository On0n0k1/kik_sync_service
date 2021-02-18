//! # Worker
//! 
//! Each thread will run an instance of a Worker type for given message traits T (Data), R (Input) and S (Message).
//! 
//! 
//! Each Worker has the receiver for the inserter "rx_inserter" channel, and also the transmitter "tx_deliverer" for the deliverer channel. 
//! 
//! 
//! This module is not meant to be used directly. But the project is free and open source, so feel free to do as you please.
//! 
//! # Panics!
//! The receivers will be "Weak Arc" + "Mutex" references for the original receiver that is held by the parent "kik_channel" type. 
//! In other words, when "kik_channel drops", workers will lose the reference and drop without panicking. 
//! But if they try to send a message to the transmitter and get a "disconnect" or "poisoned" error, they will panic.
//! 
//! 
//! # Contribute
//! There are currently no methods in kik_channel for catching dropped Workers due to panics. I, the original developer, On0n0k1, am not sure how to deal with it yet.
//! Am also open for receiving any help regarding methods for checking the worker threads for panics, reporting and/or restarting them as needed.
//! 
//! 

use std::marker::PhantomData;
use std::thread::{yield_now};
use std::sync::{Weak, Mutex, TryLockError};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, TryRecvError};

use crate::kik_message::{Message, MessageInput, MessageData};

/// Extends kik_channel. Not meant to be used individually.
pub struct Worker<T, R, S>  where 
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
{
    id: usize,
    rx_inserter: Weak<Mutex<Receiver<S>>>,
    tx_deliverer: SyncSender<S>,

    // PhantomData tells the compiler that generics T and R exist in the implementation but are not stored in the struct
    resource_type: PhantomData<T>,
    resource_type2: PhantomData<R>,
}

// Not sure how to indent this giant block
impl<T, R, S> Worker<T, R, S> where 
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
{
    /// Construct a new worker with given id, Weak Mutex Receiver and SyncSender.
    pub fn new(id: usize, rx_inserter: Weak<Mutex<Receiver<S>>>, tx_deliverer: SyncSender<S>) ->  Self
    {
        Worker{
            id: id,
            rx_inserter,
            tx_deliverer,
            // ::< used to specify type of const arguments
            resource_type: PhantomData::<T>,
            resource_type2: PhantomData::<R>,
        }
    }

    /// Get a message from the 'inserter' channel receiver. Message is sent by kik_feeder.
    fn get_message(&self) -> S{
        loop{
            yield_now();
            // turn the weak lock into a strong lock in order to access it
            match self.rx_inserter.upgrade(){
                Some(new_lock) => {
                    // if successful, try accessing the lock
                    match new_lock.try_lock(){
                        Err(err) => {
                            match err{
                                // If a thread panicked while holding the lock, this will quit.
                                TryLockError::Poisoned(_) => {
                                    panic!("Closing thread nr {} due to channel poisoning.", self.id)
                                },
                                // If access is blocked, yield remaining time for the cpu and try again.
                                TryLockError::WouldBlock => continue,
                            };
                        },
                        // if successful, try to get a message from the receiver in the lock
                        Ok(new_rx_inserter) => {
                            match new_rx_inserter.try_recv(){
                                Err(err) => {
                                    match err{
                                        // When the main feeder has finished sending and retrieving all the packages, it will disconnect the channel. 
                                        // Therefore it means it's time for the workers to close.
                                        TryRecvError::Disconnected => {
                                            std::mem::drop(self);
                                        },
                                        TryRecvError::Empty => continue,
                                    }
                                },
                                Ok(new_message) => return new_message,
                            };
                        },
                    };
                },
                // Arc reference has been dropped by the parent channel.
                None => {
                    // Main reference dropped. Worker closing
                    std::mem::drop(self);
                }
            };
        }
    }
    
    /// Send a message to the 'deliverer' channel SyncSender. Message is retrieved by kik_feeder.
    fn send_message(&self, message: S){
        loop{
            let new_message = message.clone();
            match self.tx_deliverer.try_send(new_message){
                Ok(_) => {
                    break;
                },
                Err(err) => {
                    match err{
                        TrySendError::Full(_) => {
                            yield_now();
                            continue;
                        },
                        TrySendError::Disconnected(_) => {
                            panic!("Error: Channel disconnected while sending.");
                        }
                    }
                }
            }
        }
    }

    // Thread doesn't change state while running
    /// Run continuously getting, working and retrieving messages in the channel. This is supposed to be run in a thread created by kik_channel.
    pub fn run(&self) {
        println!("Starting worker nr {}!", self.id);
        loop{
            let mut message: S = self.get_message();
            message.work();
            self.send_message(message);
        }
    }
}

impl<T, R, S> Drop for Worker<T, R, S> where 
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
{
    fn drop(&mut self){
        std::mem::drop(&self.tx_deliverer);
        std::mem::drop(&self.rx_inserter);
    }
}
