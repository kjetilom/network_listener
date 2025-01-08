
pub struct Datapoint<T> {
    pub timestamp: u64,
    pub value: T,
}

pub struct Metadata {
    pub name: String,
    pub description: String,
}

pub struct Timeseries<T> {
    pub data: Vec<Datapoint<T>>,
    pub metadata: Metadata,
}

// impl<T> Timeseries<T> {
//     fn new(name: String, description: String) -> Self {
//         Timeseries {
//             data: Vec::new(),
//             metadata: Metadata {
//                 name,
//                 description,
//             },
//         }
//     }

//     fn add(&mut self, timestamp: u64, value: T) {
//         self.data.push(Datapoint {
//             timestamp,
//             value,
//         });
//     }

//     fn add_multiple(&mut self, datapoints: Vec<Datapoint<T>>) {
//         self.data.extend(datapoints);
//     }

//     fn get_datapoints(&self, start: u64, end: u64) -> Vec<&Datapoint<T>> {
//         self.data.iter().filter(|dp| dp.timestamp >= start && dp.timestamp <= end).collect()
//     }

//     fn flush(mut self) -> Vec<Datapoint<T>> {
//         self.data.drain(..).collect()
//     }

//     pub fn get_metadata(&self) -> &Metadata {
//         &self.metadata
//     }
// }