//! # Feeder
//! 
//! Used by kik_channel for sending/retrieving messages to/from workers. Not meant to be used directly.
//! 
//! 
//! # How it works
//! It's behavior can be checked first through it's "Iterator" implementation. The user iterates through kik_channel, which calls kik_feeder's "next" method.
//! kik_feeder will check how many messages are roaming through it's system (counted through how many "gets" and "sends" were successful). If there are not enough messages,
//! it will send more in the system. If there are no messages to send and no messages to retrieve, return None.
//! 
//! If there are messages to retrieve, it will block until a worker sends it in the "deliverer" channel. A deadlock might occur if the thread panics while working.
//! So be aware that the implementation of the message relies completely on the user.
//! 
//! Once it retrieves a message from the deliverer. The feeder will call the message's implementation of clone_message_data to get a copy of the data to send back 
//! to the iterator. Before returning the message_data, it will try to reset the message that it's holding with the next input waiting to be sent back to the system. 
//! This is done to reduce calls to memory management in the system.
//! 
//! 
//! # Tips
//! Use large packs of data in each message, so there's the least number of messages possible. Less messages = less calls to the Arc pointer, which is really slow.
//! 
//! 
//! # Contribute
//! This would be optimal if instead of using memory ownership, the threads and workers focused entirely on borrows. 
//! The problem would then be code complexity that includes lifetimes. But messages could become a lot lighter if they only held references to memory, 
//! saving stack space. Maybe the code would become so complex that it should be used in another crate entirely. Not sure yet.
//! 
//! 
//! # Panics!
//! Will panic if it tries to send a message to inserter but receive a "disconnect" error. The order for drop is kik_channel then kik_feeder then kik_worker.
//! When kik_channel drops, all the others will do the same without panicking. But if channel is disconnected, then some unexpected event happened.
//! 
//! 

use std::thread::{yield_now};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, TryRecvError};
use std::marker::PhantomData;
use crate::kik_message::{MessageData, MessageInput, Message};

/// Used by kik_channel for inserting/retrieving messages. It's public, but not meant to be used directly.
pub struct FeederRecycler<T, R, S>  where 
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
{
    id: usize,
    // counts how many messages are to be recovered from the system
    messages: usize,
    // Holds how many max messages should be in the system
    package_number: usize,
    input_vec: Vec<R>,

    tx_inserter: SyncSender<S>,
    rx_deliverer: Receiver<S>,

    // PhantomData is to tell the compiler that generics T and R exist in the implementation but are not stored in the struct
    resource_type: PhantomData<T>,
    resource_type2: PhantomData<R>,
}

impl<T, R, S> FeederRecycler<T, R, S> where 
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
{
    /// Constructs a new instance of feeder with default values.
    pub fn new(id: usize, package_number: usize, tx_inserter: SyncSender<S>, rx_deliverer: Receiver<S>)->Self{
        FeederRecycler{
            id,
            input_vec: Vec::new(),
            package_number,

            messages: 0,
            tx_inserter,
            rx_deliverer,

            // ::< used to specify type of const arguments
            resource_type: PhantomData::<T>,
            resource_type2: PhantomData::<R>,            
        }
    }
    /// Append a new vec of input values to iterate later on.
    pub fn append_input(&mut self, input_vec: &mut Vec<R>){
        self.input_vec.append(input_vec);
    }

    /// Send a 'work' message to all the workers.
    fn send_message(&mut self, message: S){
        // attempt to send the message until it succeeds or the channel is closed.
        loop{
            let message_copy = message.clone();
            yield_now();
            // println!("Sending message.");
            match self.tx_inserter.try_send(message_copy){
                Ok(_) => {
                    // println!("Succesfully sent.");
                    self.messages = self.messages + 1;
                    break;
                },
                Err(err) => {
                    match err{
                        TrySendError::Full(_) => {
                            continue;
                        },
                        TrySendError::Disconnected(_) => {
                            panic!("Feeder Error(id: {}): Channel disconnected.", self.id);
                        }
                    }
                }
            }
        }
    }

    // get a result message from workers
    /// Retrieve a result message from the workers.
    fn get_message(&mut self) -> S{
        let message: S;
        loop{
            yield_now();
            // Try to retrieve a message from workers
            // println!("Retrieving message");
            let get_message = self.rx_deliverer.try_recv();
            match get_message{
                Ok(new_message) => {
                    // successful retrieval
                    // println!("Successful retrieval.");
                    message = new_message;
                    self.messages = self.messages -1;
                    break;
                },
                Err(err) => match err{
                    // If it's empty, wait and try again until all the counters were used
                    TryRecvError::Empty => {
                        continue;
                    },
                    // This thread is supposed to exit before the workers. Else something wrong went with them.
                    TryRecvError::Disconnected => panic!("Error feeder id {}: behave_inserter_deliverer can't pull messages because channel is disconnected.", self.id),
                }
            }
        }
        message
    }

    /// Returns how many messages are still to be processed and recovered. This doesn't tell how many are results waiting to be recovered and how many are still waiting for the workers. Just how many iterations might remain.
    pub fn get_remaining_messages(&mut self) -> usize{
        self.messages + self.input_vec.len()
    }

    /// Feed messages for the workers until the max number set has been achieved.
    fn feed_initial_messages(&mut self){
        for _ in (self.messages)..(self.package_number){
            // It will stop sending messages if there is no input remaining.
            let new_input: R = match self.input_vec.pop(){
                Some(x) => x,
                // No more messages to send.
                None => break,
            };
            let mut new_message: S = S::new();
            new_message.set_input(new_input);

            self.send_message(new_message);
        }
    }

    /// Get a message from the workers and pull a copy of the MessageData inside. If there are more messages to sent, it will recycle the acquired message for the workers. Saving time.
    fn retrieve_data(&mut self)-> Option<T>{
        let new_data: T;
        match self.input_vec.pop(){
            // This means that there are no more messages to send
            None => {
                // This means that there are no more messages to get
                if self.messages == 0{
                    // ending function or iteration
                    return None;
                }
                
                // This means that there are no messages to send, but there are messages to retrieve.
                let new_message = self.get_message();
                new_data = new_message.clone_message_data();
                // There's no need to recycle more messages, therefore new_message will be dropped. This needs to be done, since each message lifetime is 'static. Or else memory will only be freed when program ends (I think).
                std::mem::drop(new_message);
                return Some(new_data);
            },

            //This means that there are still messages to send
            Some(new_input) => {                
                // Considering the special case where there is only one input remaining (the one currently held in 'new_input') no more messages to get, no more messages to send. 
                // In this case, a message will be created, sent, and consumed, instead of recycled.
                if self.messages == 0{
                    let mut new_message = S::new();
                    new_message.set_input(new_input);
                    self.send_message(new_message);
                    // checks to send a few input messages if possible. While worker process the first message.
                    self.feed_initial_messages();
                    let new_message = self.get_message();
                    new_data = new_message.clone_message_data();
                    std::mem::drop(new_message);
                    // checks to send another message for the workers since this one had to be deleted.
                    self.feed_initial_messages();
                    return Some(new_data);
                }

                // This means that there are less messages in the delivery system than there should be.
                if self.messages < self.package_number {
                    // Send new messages until the system max has been reached.
                    self.feed_initial_messages();
                }

                // There are messages to feed and there are messages to get. Therefore recycle messages.
                let mut new_message = self.get_message();
                new_data = new_message.clone_message_data();
                
                // Data will be replaced by the workers. Only thing they need is the input.
                new_message.set_input(new_input);
                self.send_message(new_message);
                return Some(new_data);
            }
        }
    }
}

// This will be used by the channel that handles the feeder. Call kik_channel's iterator instead.
impl<T, R, S> Iterator for FeederRecycler<T, R, S>  where 
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
// S: Message<T, R> + Sync + Send + Copy + 'static,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        // Returns None if there are no messages to retrieve, ending the iteration.
        // Unless the entire object goes out of scope, we can keep feeding more input to use in other iterations later on.
        self.retrieve_data()
    }
}

impl<T, R, S> Drop for FeederRecycler<T, R, S> where
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
{
    fn drop(&mut self){
        std::mem::drop(&self.input_vec);
        std::mem::drop(&self.rx_deliverer);
        std::mem::drop(&self.tx_inserter);
    }
}