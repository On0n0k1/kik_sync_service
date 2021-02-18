//! # Messages
//! 
//! Traits for defining how the channel should transfer, work and retrieve the messages.
//! Users have to implement these traits in their data structure before using the *DeliveryService*.
//! 
//! The type with the *Message* trait must be able to hold types that implement *MessageData* and *MessageInput* as well.
//! 
//! *Message* is what the channels share. *MessageInput* is what *FeederRecycler* sets in each *Message*.
//! *MessageData* is what the channel returns when the user iterates through it.
//! 
//! The *MessageData* shared must be *Sync* and *Send*. Must have *'static* lifetimes, must have *Clone* trait, 
//! but doesn't need to be *Copy*. I haven't tested if being *Copy* will break *Drop* behaviors.
//! But if it did, compiler would probably notice.
//! 
//! 

use std::marker::{Send, Sync};

// Making sure that this trait only applies to objects that have Clone
/// MessageData holds the resource type that will be returned by the worker-threads. Must implement Sync, Send, Clone and have lifetime 'static.
pub trait MessageData: Sync + Send + Clone + 'static{
    fn new() -> Self;
}

// This is the trait input that can only be applied to ojbects with MessageData trait
/// MessageInput will have the input arguments for generating each MessageData. Must implement Sync, Send, Clone and have lifetime 'static.
pub trait MessageInput<T> : Sync + Send + Clone + 'static where T: MessageData
{
    fn new() -> Self;
}

// This is the Message Trait that holds the data and the value type that changes it
/// Message has the tools to generate each MessageData T, based on each MessageInput R. Must implement Sync, Send, Clone and have lifetime 'static.
pub trait Message<T, R> : Sync + Send + Clone + 'static where
                                                R: MessageInput<T>,
                                                T: MessageData,
{
    /// Behavior for storing a given input MessageInput, before a worker can use it for generating MessageData. Used by kik_feeder.
    fn set_input(&mut self, message_input: R);

    /// Workers will call this to use the stored MessageInput (R<T>) to generate and replace the existing MessageData stored. Used by kik_worker.
    fn work(&mut self);

    /// This will call MessageInput::new() method. No need to implement this. Used by kik_feeder.
    fn new_message_input() -> R{
        R::new()
    }

    /// This will call MessageData::new() method. No need to implement this. Used by kik_feeder.
    fn new_message_data() -> T{
        T::new()
    }

    /// This method is used when retrieving MessageData for the iterator. Clone the MessageData stored and return it. Used by kik_feeder.
    fn clone_message_data(&self) -> T;
    
    /// Construct a new message with default values. Used by kik_feeder.
    fn new() -> Self;

}