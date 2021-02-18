# kik_sync_service
Synchronous Rust library for executing customized multi-threaded operations.

[link for web docs](https://on0n0k1.github.io/Projects/Rust%20crates/kik_sync_service/doc/kik_sync_service/index.html)
[link for crates page](https://crates.io/crates/kik_sync_service)

This is just a crate that creates two channels and several
 workers for synchronous operations. The catch is that the 
 messages are built by the user. Make a struct to work as the 
 data you recover (MessageData), a struct to work as the input 
 you need (MessageInput), and include them in the message that 
 you will share between the threads (Message).

## How to use

The traits that the user needs to implement are here:

'''
use std::marker::{Send, Sync};

/// MessageData holds the resource type that will be returned 
/// by the worker-threads. Must implement Sync, Send, Clone 
/// and have lifetime 'static.
pub trait MessageData: Sync + Send + Clone + 'static{
    fn new() -> Self;
}

// This is the trait input that can only be applied to objects 
// with MessageData trait
/// MessageInput will have the input arguments for generating 
/// each MessageData. Must implement Sync, Send, Clone and have 
/// lifetime 'static.
pub trait MessageInput<T> : Sync + Send + Clone + 'static where T: MessageData
{
    fn new() -> Self;
}

// This is the Message Trait that holds the data and the 
// value type that changes it
/// Message has the tools to generate each MessageData T, 
/// based on each MessageInput R. Must implement Sync, 
/// Send, Clone and have lifetime 'static.
pub trait Message<T, R> : Sync + Send + Clone + 'static where
R: MessageInput<T>,
T: MessageData,
{
    /// Behavior for storing a given input MessageInput, 
    /// before a worker can use it for generating MessageData. 
    /// Used by kik_feeder.
    fn set_input(&mut self, message_input: R);

    /// Workers will call this to use the stored 
    /// MessageInput (R<T>) to generate and replace 
    /// the existing MessageData stored. Used by kik_worker.
    fn work(&mut self);

    /// This will call MessageInput::new() method. No 
    /// need to implement this. Used by kik_feeder.
    fn new_message_input() -> R{
        R::new()
    }

    /// This will call MessageData::new() method. 
    /// No need to implement this. Used by kik_feeder.
    fn new_message_data() -> T{
        T::new()
    }

    /// This method is used when retrieving MessageData 
    /// for the iterator. Clone the MessageData stored 
    /// and return it. Used by kik_feeder.
    fn clone_message_data(&self) -> T;
    
    /// Construct a new message with default values. 
    /// Used by kik_feeder.
    fn new() -> Self;

}

'''


After the types are implemented. Construct a new channel by calling 

DeliveryService::default(), 

feed a vector of inputs by calling 

DeliveryService::feed_feeder(&mut self, input_vec: &mut Vec<R>)

and iterate over &mut delivery_service to get the MessageData you implemented.

Takes around 100 lines to set all the types. 
But once they are set, only around 5-10 lines to use any number of times needed.

## Giant Example

Here is an example of the crate being used:

'''


#[cfg(test)]
mod tests{
    use crate::message::{Message, MessageData, MessageInput};
    use crate::channel::{DeliveryService};
    // use crate::*;

    // What type of data should be returned.
    pub struct MessageArray{
        data: [u32; 1024],
    }

    impl Clone for MessageArray{
        fn clone(&self) -> Self{
            let mut new_array: [u32; 1024] = [0; 1024];
            for i in 0..1024{
                new_array[i] = self.data[i];
            }
            MessageArray{
                data: new_array,
            }
        }
    }

    // with this trait it can be used as data for a Message.
    impl MessageData for MessageArray{
        fn new() -> Self{
            MessageArray{
                data: [0; 1024],
            }
        }
    }

    impl MessageArray{
        pub fn get(&mut self) -> &mut [u32; 1024]{
            &mut self.data
        }
    }


    // What kind of input it needs.
    pub struct Coordinates{
        pub x0: usize,
        pub y0: usize,
        pub x1: usize,
        pub y1: usize,
    }

    // Doesn't need to implement Copy, but needs Clone.
    impl Clone for Coordinates{
        fn clone(&self) -> Self{
            Coordinates{
                x0: self.x0,
                y0: self.y0,
                x1: self.x1,
                y1: self.y1,
            }
        }
    }

    // This implementation tells the compiler that this object can be 
    // used as input for the worker threads, and it can only work with MessageArray.
    impl MessageInput<MessageArray> for Coordinates{
        fn new() -> Self{
            Coordinates{
                x0: 0,
                y0: 0,
                x1: 0,
                y1: 0,
            }
        }
    }


    // This is the message that holds both the data and input. 
    // Feel free to add anything else you might need to work with it.
    pub struct ThreadMessage{
        pub array: MessageArray,
        pub current_input: Coordinates,
    }

    impl Clone for ThreadMessage{
        fn clone(&self) -> Self{
            ThreadMessage{
                array: self.array.clone(),
                current_input: self.current_input.clone(),
            }
        }
    }

    // ThreadMessage uses MessageArray as data,
    // ThreadMessage uses Coordinates as input to change the data.
    impl Message<MessageArray, Coordinates> for ThreadMessage {

        fn set_input(&mut self, message_input: Coordinates){
            self.current_input = message_input.clone();
        }

        fn work (&mut self){
            let (x0, y0, x1, y1) = (
                self.current_input.x0,
                self.current_input.y0,
                self.current_input.x1,
                self.current_input.y1,
            );

            let array = self.array.get();
            let mut counter: usize = 0;

            // Not very creative right now, each operation will count from 0 to 1024.
            // This is just to show that the results are being made and returned. I'll use it to generate fractals in the next project.
            // I'm thankful for anyone willing to offer a better example for this later.
            // Counting in the first line will go like 0 1 2 3 ... 30 31 1 2 3 ...
            // Counting in the second line will go like 32 33 34 35 ... 59 60 61 62 63 ...
            // And so forth until 1023 in the last line.
            for _y in (y0)..(y1){
                for _x in (x0)..(x1){
                    let value = counter;
                    array[counter] = value as u32;

                    counter = counter + 1;
                }
            }
        }

        fn clone_message_data(&self) -> MessageArray{
            self.array.clone()
        }

        fn new() -> Self{
            let new_data = MessageArray::new();
            let new_input = Coordinates::new();

            ThreadMessage{
                current_input: new_input,
                array: new_data,
            }
        }
        
    }

    // Finally, Now that all the data structure is set, time to use the channel.
    #[test]
    fn test(){
        let width: usize = 1024;
        let height: usize = 768;
        let mut coordinates: Vec<Coordinates> = Vec::with_capacity((height as f32/32.0 * width as f32/32.0)as usize);
        assert_eq!(width % 32, 0);
        assert_eq!(height % 32, 0);

        // Creating a vec of coordinates to use as input.
        // for y in 0..24
        for y in 0..(((height as f32)/32.0) as usize){
            // for x in 0..32
            for x in 0..(((width as f32)/32.0) as usize){
                let (x0, y0) = (32 * x, 32 * y);
                coordinates.push(Coordinates{x0: x0, y0: y0, x1: x0 + 32, y1: y0 + 32});
            }
        }
        // Personal Note:
        // create a vec of inputs
        // create channel
        // send the vec of inputs
        // iterate through the channel
        // print the resulting values

        //data is MessageArray
        //input is Coordinates
        //message is ThreadMessage

        // Creating a channel that uses MessageArray as MessageData, Coordinates as MessageInput, ThreadMessage as Message. Default config values have been used.
        let mut kiki_channel: DeliveryService<MessageArray,Coordinates,ThreadMessage> = DeliveryService::default();
        kiki_channel.feed_feeder(&mut coordinates);

        let mut counter = 0;
        // Need to iterate through a mutable reference of kiki_channel to maintain ownership of it.
        for mut i in &mut kiki_channel{
            let mut highest: u32 = 0;
            let message_array = i.get();
            for j in message_array{
                if highest < *j {
                    highest = *j;
                }
            }
            // All the highest values for each line will be 31, 63, n * 32 -1, ...
            assert_eq!(highest % 32, 31);
            println!("Total line {}: {}", counter, highest);
            counter += 1;
        }

        // Creating another vec to feed the structure again.
        // for y in 0..24
        for y in 0..(((height as f32)/32.0) as usize){
            // for x in 0..32
            for x in 0..(((width as f32)/32.0) as usize){
                let (x0, y0) = (32 * x, 32 * y);
                coordinates.push(Coordinates{x0: x0, y0: y0, x1: x0 + 32, y1: y0 + 32});
            }
        }
        
        // You can feed more input values after emptying the results from last run.
        kiki_channel.feed_feeder(&mut coordinates);

        let mut counter = 0;
        // The worker threads and feeder will only be closed when channel goes out of scope (unless they panic).
        // Need to iterate through a mutable reference of kiki_channel to maintain ownership of it.
        for mut i in &mut kiki_channel{
            let mut highest: u32 = 0;
            let message_array = i.get();
            for j in message_array{
                if highest < *j {
                    highest = *j;
                }
            }
            // Used this when I was testing as fn main
            // if counter % 13 == 0{
            //     println!("Total linha {}: {}", counter, total);
            // }

            // All the highest values for each line will be 31, 63, n * 32 -1, ...
            assert_eq!(highest % 32, 31);
            println!("Total line {}: {}", counter, highest);
            counter += 1;
        }
    }
}

'''


## Contribute

There are some sections asking for help contributing. The main function of this crate worked properly for me, but there are many things yet to improve. The details are better explained in the web docs: https://on0n0k1.github.io/Projects/Rust%20crates/kik_sync_service/doc/kik_sync_service/index.html 