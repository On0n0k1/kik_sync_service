//! # kik_sync_service
//! 
//! A synchronous work system for dealing with small inputs that lead to heavy workloads.
//! 
//! 
//! ## Why Use This
//! 
//! This "*Message* + *worker* thread" system was built for the following situations:
//! 
//! - When there is a small sample of inputs that can generate a very large amount of data.
//! 
//! - When a small input can generate a very heavy workload.
//! 
//! - When there is no need to store the data generated in the memory.
//! 
//! - When a system must process a stream of data and send it directly somewhere else. 
//! 
//! - I.E. sound processing, image processing, handling network requests, etc.
//! 
//! - When generating individual frames for some GUI.
//! 
//! 
//! The system consists of several *Worker*s and a single thread (*FeederRecycler*) sending and retrieving messages. If the (work time)/message is high enough,
//! this single thread handling input/output might have enough time to store data in the memory without slowing down the workers. 
//! It's up to the creativity of each user.
//! 
//! # Example
//! 
//! This is huge because we need to first create a struct that implements *MessageData*, then another for *MessageInput*, then another for *Message*.
//! 
//! But what kind of data and how it's worked is all up to the user. Just create the *DeliveryService* channel, 
//! use default values or a custom *ChannelConfig* argument. Then create a vector of the input data you need. 
//! Feed that data into the *DeliveryService* by calling "*feed_feeder*" method. As soon as data starts being fed *Worker*s will work.
//! Finally start getting the results with a "for" call to a mutable reference of the channel. 
//! Repeat feed and iterate as many times as needed.
//! 
//! 
//!     use kik_sync_service::message::{Message, MessageData, MessageInput};
//!     use kik_sync_service::channel::{DeliveryService};
//!     
//! 
//!     // What type of data should be returned.
//!     pub struct MessageArray{
//!         data: [u32; 1024],
//!     }
//! 
//!     impl Clone for MessageArray{
//!         fn clone(&self) -> Self{
//!             let mut new_array: [u32; 1024] = [0; 1024];
//!             for i in 0..1024{
//!                 new_array[i] = self.data[i];
//!             }
//!             MessageArray{
//!                 data: new_array,
//!             }
//!         }
//!     }
//!
//!     // with this trait it can be used as data for a Message.
//!     impl MessageData for MessageArray{
//!         fn new() -> Self{
//!             MessageArray{
//!                 data: [0; 1024],
//!             }
//!         }
//!     }
//!
//!     impl MessageArray{
//!         pub fn get(&mut self) -> &mut [u32; 1024]{
//!             &mut self.data
//!         }
//!     }
//!
//!
//!     // What kind of input it needs.
//!     pub struct Coordinates{
//!         pub x0: usize,
//!         pub y0: usize,
//!         pub x1: usize,
//!         pub y1: usize,
//!     }
//!
//!     // Doesn't need to implement Copy, but needs Clone.
//!     impl Clone for Coordinates{
//!         fn clone(&self) -> Self{
//!             Coordinates{
//!                 x0: self.x0,
//!                 y0: self.y0,
//!                 x1: self.x1,
//!                 y1: self.y1,
//!             }
//!         }
//!     }
//!
//!     // This implementation tells the compiler that this object can be 
//!     // used as input for the worker threads, and it can only work with MessageArray.
//!     impl MessageInput<MessageArray> for Coordinates{
//!         fn new() -> Self{
//!             Coordinates{
//!                 x0: 0,
//!                 y0: 0,
//!                 x1: 0,
//!                 y1: 0,
//!             }
//!         }
//!     }
//!
//!
//!     // This is the message that holds both the data and input. 
//!     // Feel free to add anything else you might need to work with it.
//!     pub struct ThreadMessage{
//!         pub array: MessageArray,
//!         pub current_input: Coordinates,
//!     }
//!
//!     impl Clone for ThreadMessage{
//!         fn clone(&self) -> Self{
//!             ThreadMessage{
//!                 array: self.array.clone(),
//!                 current_input: self.current_input.clone(),
//!             }
//!         }
//!     }
//!
//!     // ThreadMessage uses MessageArray as data,
//!     // ThreadMessage uses Coordinates as input to change the data.
//!     impl Message<MessageArray, Coordinates> for ThreadMessage {
//!
//!         fn set_input(&mut self, message_input: Coordinates){
//!             self.current_input = message_input.clone();
//!         }
//!
//!         fn work (&mut self){
//!             let (x0, y0, x1, y1) = (
//!                 self.current_input.x0,
//!                 self.current_input.y0,
//!                 self.current_input.x1,
//!                 self.current_input.y1,
//!             );
//! 
//!             let array = self.array.get();
//!             let mut counter: usize = 0;
//!
//!             // Not very creative right now, each operation will count from 0 to 1024.
//!             // This is just to show that the results are being made and returned. I'll use it to generate fractals in the next project.
//!             // I'm thankful for anyone willing to offer a better example for this later.
//!             // Counting in the first line will go like 0 1 2 3 ... 30 31 1 2 3 ...
//!             // Counting in the second line will go like 32 33 34 35 ... 59 60 61 62 63 ...
//!             // And so forth until 1023 in the last line.
//!             for _y in (y0)..(y1){
//!                 for _x in (x0)..(x1){
//!                     let value = counter;
//!                     array[counter] = value as u32;
//!
//!                     counter = counter + 1;
//!                 }
//!             }
//!         }
//!
//!         fn clone_message_data(&self) -> MessageArray{
//!             self.array.clone()
//!         }
//!
//!         fn new() -> Self{
//!             let new_data = MessageArray::new();
//!             let new_input = Coordinates::new();
//!
//!             ThreadMessage{
//!                 current_input: new_input,
//!                 array: new_data,
//!             }
//!         }
//!        
//!     }
//!
//!     // Finally, Now that all the data structure is set, time to use the channel.
//!     #[test]
//!     fn test(){
//!         let width: usize = 1024;
//!         let height: usize = 768;
//!         let mut coordinates: Vec<Coordinates> = Vec::with_capacity((height as f32/32.0 * width as f32/32.0)as usize);
//!         assert_eq!(width % 32, 0);
//!         assert_eq!(height % 32, 0);
//!
//!         // Creating a vec of coordinates to use as input.
//!         // for y in 0..24
//!         for y in 0..(((height as f32)/32.0) as usize){
//!             // for x in 0..32
//!             for x in 0..(((width as f32)/32.0) as usize){
//!                 let (x0, y0) = (32 * x, 32 * y);
//!                 coordinates.push(Coordinates{x0: x0, y0: y0, x1: x0 + 32, y1: y0 + 32});
//!             }
//!         }
//!         // Personal Note:
//!         // create a vec of inputs
//!         // create channel
//!         // send the vec of inputs
//!         // iterate through the channel
//!         // print the resulting values
//!
//!         //data is MessageArray
//!         //input is Coordinates
//!         //message is ThreadMessage
//!
//!         // Creating a channel that uses MessageArray as MessageData, Coordinates as MessageInput, ThreadMessage as Message. Default config values have been used.
//!         let mut kiki_channel: DeliveryService<MessageArray,Coordinates,ThreadMessage> = DeliveryService::default();
//!         kiki_channel.feed_feeder(&mut coordinates);
//!
//!         let mut counter = 0;
//!         // Need to iterate through a mutable reference of kiki_channel to maintain ownership of it.
//!         for mut i in &mut kiki_channel{
//!             let mut highest: u32 = 0;
//!             let message_array = i.get();
//!             for j in message_array{
//!                 if highest < *j {
//!                     highest = *j;
//!                 }
//!            }
//!            // All the highest values for each line will be 31, 63, n * 32 -1, ...
//!             assert_eq!(highest % 32, 31);
//!             println!("Total line {}: {}", counter, highest);
//!             counter += 1;
//!         }
//!
//!         // Creating another vec to feed the structure again.
//!         // for y in 0..24
//!         for y in 0..(((height as f32)/32.0) as usize){
//!             // for x in 0..32
//!             for x in 0..(((width as f32)/32.0) as usize){
//!                 let (x0, y0) = (32 * x, 32 * y);
//!                 coordinates.push(Coordinates{x0: x0, y0: y0, x1: x0 + 32, y1: y0 + 32});
//!             }
//!         }
//!        
//!         // You can feed more input values after emptying the results from last run.
//!         kiki_channel.feed_feeder(&mut coordinates);
//!
//!         let mut counter = 0;
//!         // The worker threads and feeder will only be closed when channel goes out of scope (unless they panic).
//!         // Need to iterate through a mutable reference of kiki_channel to maintain ownership of it.
//!         for mut i in &mut kiki_channel{
//!             let mut highest: u32 = 0;
//!             let message_array = i.get();
//!             for j in message_array{
//!                 if highest < *j {
//!                     highest = *j;
//!                 }
//!             }
//!             // Used this when I was testing as fn main
//!             // if counter % 13 == 0{
//!             //     println!("Total linha {}: {}", counter, total);
//!             // }
//!
//!             // All the highest values for each line will be 31, 63, n * 32 -1, ...
//!             assert_eq!(highest % 32, 31);
//!             println!("Total line {}: {}", counter, highest);
//!             counter += 1;
//!         }
//!     }
//! 
//! 

// This crate is a library
#![crate_type = "lib"]
// The library is named "kik_sync_service"
#![crate_name = "kik_sync_service"]

mod kik_message;
mod kik_channel;
mod kik_worker;
mod kik_feeder;
mod kik_message_example;

/// Holds the traits used for message sharing and how to work them. They must be manually set by the user before using channel.
pub mod message{
    pub use crate::kik_message::{Message,MessageInput, MessageData};
}

/// DeliveryService is the channel used for the synchronous message-sharing and work. It can be created with DeliveryService::default values or be customized by using ChannelConfig as argument for DeliveryService::new.
pub mod channel{
    pub use crate::kik_channel::{ChannelConfig, DeliveryService};
}
