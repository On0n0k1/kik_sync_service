//! # kik_channel
//! 
//! 


use std::thread::JoinHandle;
use std::default::Default;
use std::marker::PhantomData;

// use std::thread;
use std::thread::{Builder};
use std::sync::{Arc, Mutex, TryLockError};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

use crate::kik_message::{Message, MessageInput, MessageData};
use crate::kik_worker::Worker;
use crate::kik_feeder::FeederRecycler;

/// To be used when the user needs to set specific configurations before creating a *DeliveryService* channel. Optional type.
/// 
/// # How to use it
/// 
/// - Create a new instance using *ChannelConfig::default()*
/// 
/// - Set custom values for it.
/// 
/// - Construct a new *DeliveryService* instance using *DeliveryService::new(your_channel_config_name)*
/// 
/// # Methods
/// 
/// Be wary that some sets will change others. The titles above the list might change the values of the ones below.
/// 
/// 
/// 

pub struct ChannelConfig{
    stack_size: usize,
    worker_number: usize,
    package_number: usize,
    channel_size: usize,
}

impl Default for ChannelConfig{
    fn default() -> Self {
        // I wanted to know how to set this to the number of cores in the cpu. Currently I don't know how.
        let worker_number: usize = 8;
        let channel_size: usize = worker_number;
        let package_number: usize = channel_size * 2;

        ChannelConfig{
            // This is the default in Rust as of today feb-15-2021
            stack_size: 2 * 1024 * 1024,
            worker_number,
            channel_size,
            package_number,
        }
    }
}



impl ChannelConfig{
    /// Create new ChannelConfig with default values.
    pub fn new() -> Self{
        Self::default()
    }

    /// Set worker number. Package_number will be set to twice the value. Panics if less than 1. Default value is 8.
    /// Changing worker number changes channel size to the same value. Also change package number to twice the value.
    pub fn set_worker_number(&mut self, worker_number: usize){
        if worker_number < 1{
            panic!("Error ChannelConfig::set_worker_number: There must be at least one worker thread (currently {}).", worker_number);
        }
        self.worker_number = worker_number;
        self.channel_size = worker_number;
        self.package_number = worker_number * 2;
    }

    /// Set the number of packages roaming in the delivery system. Minimum value is worker_number + 1. Panics if value is invalid. Default is channel_size * 2.
    pub fn set_package_number(&mut self, package_number: usize){
        if package_number <= self.worker_number{
            panic!("There's not enough packages for every worker to use.");
        }
        self.package_number = package_number;
    }

    /// Set stack size for each of the workers. The new size will not be evaluated. Responsibility for the value relies on the user. Default 2 * 1024 * 1024.
    pub fn set_stack_size(&mut self, new_stack_size: usize){
        self.stack_size = new_stack_size;
    }

    // get functions for each value
    /// Get stored stack_size configuration to use in a new kik_channel.
    pub fn get_stack_size(&self) -> usize{
        self.stack_size
    }

    /// Get how many threads will be generated by the channel.
    pub fn get_worker_number(&self) -> usize{
        self.worker_number
    }

    /// get how many messages will be sent around the delivery system. This is reset automatically after setting worker_number.
    pub fn get_package_number(&self) -> usize{
        self.package_number
    }

    /// Get how many messages can be stored in each of the channels. Size will be automatically set whenever set_worker_number is called.
    pub fn get_channel_size(&self) -> usize{
        self.channel_size
    }

}


/// Main structure for the entire crate. Creates the channels, workers and feeder.
/// 
/// How to use it:
/// 
/// - Call *DeliveryService<T, R, S>::default()* to construct an instance for given **T**, **R**, **S** traits that use default values.
/// 
/// - Feed initial values with *DeliveryService<T, R, S>::feed_feeder(input_vec: &mut Vec<R>)*
/// 
/// - Iterate through a mutable reference of this object. "*for i in &mut delivery_service{}*"
/// 
/// - Feed more values and iterate again to get more **T** results.
/// 
/// 
/// 

pub struct DeliveryService<T, R, S>  where 
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
{
    stack_size: usize,
    worker_number: usize,
    last_id: usize,
    // () is the return value for each worker (which is nothing).
    thread_vec: Vec<JoinHandle<()>>,
    // Send and retrieve messages for the workers. Has tx_inserter and rx_deliverer channels.
    feeder: FeederRecycler<T, R, S>,

    // What the workers use.
    rx_inserter: Arc<Mutex<Receiver<S>>>,
    tx_deliverer: SyncSender<S>,

    // Tells compiler that this data exists here, but is not a type stored in the struct.
    resource_type: PhantomData<T>,
    resource_type2: PhantomData<R>,
    resource_type3: PhantomData<S>,
}


impl<T, R, S> DeliveryService <T, R, S> where 
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
{
    /// Create a new DeliveryService instance using details set in ChannelConfig. If there's no need to set specific configuration, call DeliveryService::default() instead.
    fn new(config: ChannelConfig) -> Self{
        let stack_size = config.get_stack_size();
        let worker_number = config.get_worker_number();
        let thread_vec: Vec<JoinHandle<()>> = Vec::with_capacity(worker_number);

        let channel_size = config.get_channel_size();
        let package_number = config.get_package_number();

        // Setting both channels. There are several receivers (the workers) for the inserter channel. Therefore it needs to be coupled together with an arc + Mutex reference.
        let (tx_inserter, rx_inserter) = sync_channel(channel_size);
        let rx_inserter = Arc::new(Mutex::new(rx_inserter));
        let (tx_deliverer, rx_deliverer) = sync_channel(channel_size);

        // feeder manages both sending and receiving worker messages
        let feeder: FeederRecycler<T, R, S> = FeederRecycler::new(0, package_number, tx_inserter, rx_deliverer);

        DeliveryService{
            stack_size,
            worker_number,
            last_id: 0,
            thread_vec,
            feeder,

            // Not used(yet)
            // channel_size,
            // package_number,
        
            // What the workers use
            rx_inserter,
            tx_deliverer,
        
            // Tells compiler that this data exists here, but is not a type stored in the struct.
            resource_type: PhantomData::<T>,
            resource_type2: PhantomData::<R>,
            resource_type3: PhantomData::<S>,
        }
    }

    /// Borrows a vector of inputs and append the values into the feeder. Borrowed vector will become empty.
    pub fn feed_feeder(&mut self, input_vec: &mut Vec<R>){
        self.feeder.append_input(input_vec);
    }

    /// Tells how many values are still to be recovered. Includes messages that haven't been worked yet.
    pub fn len(&mut self)-> usize{
        self.feeder.get_remaining_messages()
    }

    /// Builds and append new workers until the max set value is reached.
    fn build_workers(&mut self){
        for _ in (self.thread_vec.len())..(self.worker_number){
            self.last_id += 1;
            let new_id = self.last_id;
            
            // let new_worker: Worker<'a, T, R, S> = Worker::new(self.last_id, new_rx_inserter, new_tx_deliverer);
            let mut new_builder = Builder::new();
            new_builder = new_builder.stack_size(self.stack_size);
            new_builder = new_builder.name(format!("Worker {}", self.last_id));

            // Creating a weak reference so that it gets disconnected when the main reference (in this struct) is dropped.
            let new_rx_inserter = Arc::downgrade(&self.rx_inserter);
            let new_tx_deliverer = SyncSender::clone(&self.tx_deliverer);
            
            self.thread_vec.push(new_builder.spawn(
                move || {
                    let new_worker: Worker<T, R, S> = Worker::new(new_id, new_rx_inserter, new_tx_deliverer);
                    new_worker.run();
                    drop(new_worker);
                }
            ).unwrap());
        }
    }

}

/// Creates new DeliveryService with default values. Useful for those in a hurry.
impl<T, R, S> Default for DeliveryService<T, R, S> where 
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
{
    fn default() -> Self{
        let new_config = ChannelConfig::default();
        DeliveryService::new(new_config)
    }
}

impl<T, R, S> Iterator for &mut DeliveryService<T, R, S>  where 
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        // This will only create workers if there is less than the required number in the vector.
        self.build_workers();
        // feeder will try to get a message and return the value. Returns None if there are no messages remaining.
        self.feeder.next()
    }
}

impl<T, R, S> Drop for DeliveryService<T, R, S> where 
T: MessageData + 'static,
R: MessageInput<T> + 'static,
S: Message<T, R> + Sync + Send + Clone + 'static,
{
    fn drop(&mut self){
        loop{
            match self.rx_inserter.try_lock(){
                Ok(lock) => {
                    std::mem::drop(&lock);
                    break;
                },
                Err(err) => {
                    match err{
                        TryLockError::Poisoned(_) => {
                            break;
                        },
                        // try again later
                        TryLockError::WouldBlock => {},
                    }
                }
            }
        }
        std::mem::drop(&self.rx_inserter);
        std::mem::drop(&self.feeder);
        std::mem::drop(&self.tx_deliverer);
        std::mem::drop(&self.thread_vec);
    }
}