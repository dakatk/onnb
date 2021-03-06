use super::activations::ActivationFn;
use super::costs::Cost;
use super::metrics::Metric;
use super::optimizers::Optimizer;
use super::layer::Layer;

use ndarray::Array2;

use rand::{prelude::ThreadRng, seq::SliceRandom};
use rand::thread_rng;

use serde::ser::{Serialize, SerializeStruct, Serializer};

pub struct Network {
    /// Input, hidden, and output layers. Each layer is considered
    /// to be 'connected' to the next one in the list
    layers: Vec<Layer>,

    /// Loss function for error reporting/backprop
    cost: Box<dyn Cost>
}

impl Network {
    /// # Arguments:
    ///
    /// * `cost` - Loss function for error reporting/backprop
    pub fn new(cost: Box<dyn Cost>) -> Network {
        Network {
            layers: vec![],
            cost
        }
    }

    /// Creates a new layer and adds it to the Network. Used only for the
    /// first layer added, which is treated as the input layer
    ///
    /// # Arguments
    ///
    /// * `neurons` - Number of neurons, determines how many weights/biases
    /// are present in the new Layer
    /// * `inputs` - Size of expected the Layer's input vector
    /// * `activation_fn` - Function that determines the activation of individual neurons
    fn add_input_layer(
        &mut self,
        neurons: usize,
        inputs: usize,
        activation_fn: Box<dyn ActivationFn>,
        dropout: Option<f32>
    ) {
        self.layers.push(Layer::new(neurons, inputs, activation_fn, dropout));
    }

    /// Same as `add_input_layer`, but used for any other layer after. The number of
    /// inputs for a hidden layer is equal to the number of neurons in the preceding layer
    ///
    /// # Arguments
    ///
    /// * `neurons` - Number of neurons, determines how many weights/biases
    /// are present in the new Layer
    /// * `activation_fn` - Function that determines the activation of individual neurons
    fn add_hidden_layer(
        &mut self, 
        neurons: usize, 
        activation_fn: Box<dyn ActivationFn>, 
        dropout: Option<f32>
    ) {
        let prev_neurons = self.layers.last_mut().unwrap().neurons;

        self.layers
            .push(Layer::new(neurons, prev_neurons, activation_fn, dropout));
    }

    /// Add a Layer to the next open spot in the Network's structure. This function
    /// also dynamically expands the Network's overall size
    ///
    /// # Arguments
    ///
    /// * `neurons` - Number of neurons, determines how many weights/biases
    /// are present in the new Layer
    /// * `inputs` (optional) - Size of expected the Layer's input vector
    /// * `activation_fn` - Function that determines the activation of individual neurons
    pub fn add_layer(
        &mut self,
        neurons: usize,
        inputs: Option<usize>,
        activation_fn: Box<dyn ActivationFn>,
        dropout: Option<f32>
    ) {
        match inputs {
            Some(inputs) => self.add_input_layer(neurons, inputs, activation_fn, dropout),
            _ => self.add_hidden_layer(neurons, activation_fn, dropout)
        }
    }

    /// Trains the entire Network for a specified number of cycles. Training is
    /// stopped when the given metric is satisfied based on the input/output
    /// sets provided
    ///
    /// # Arguments
    ///
    /// * `inputs` - Set of all input vectors to train the Network on
    /// * `outputs` - Set of corresponding output vectors
    /// * `optimizer` - Optimization method used to perform perform gradient descent
    /// * `metric` - Decides when the Network is performing 'good enough'
    /// on the provided data
    /// * `epochs` - Maximum number of training cycles
    pub fn fit(
        &mut self,
        inputs: &[Array2<f64>],
        outputs: &[Array2<f64>],
        mut optimizer: Box<dyn Optimizer>,
        metric: Box<dyn Metric>,
        epochs: u64
    ) -> Vec<Array2<f64>> {
        let mut rng = thread_rng();
        for epoch in 1..=epochs {
            let mut early_stop = true;
            let mut samples: Vec<usize> = (0..inputs.len()).collect();

            samples.shuffle(&mut rng);

            for sample in samples {
                let network_output: Array2<f64> = self.predict(&inputs[sample], Some(rng));

                if !metric.call(&network_output, &outputs[sample]) {
                    early_stop = false;
                }
                
                let len: usize = self.layers.to_owned().len();
                let mut attached_layer: Option<Layer>;

                for i in (0..len).rev() {
                    {
                        attached_layer = if i < len - 1 {
                            let layer = self.layers[i + 1].clone();
                            Some(layer)
                        } else {
                            None
                        };
                    }

                    self.layers[i].back_prop(
                        &network_output,
                        &outputs[sample],
                        attached_layer,
                        self.cost.clone()
                    );
                }
                optimizer.update(&mut self.layers);
            }

            if early_stop {
                println!("Training ended on epoch {}", epoch);
                break;
            }
        }

        let mut errors: Vec<Array2<f64>> = vec![];

        for (input, output) in inputs.iter().zip(outputs) {
            let error = {
                let network_output = self.predict(input, None);
                self.cost.prime(&network_output, output)
            };

            errors.push(error);
        }

        errors
    }

    /// Performs the feedforward step for all Layers to return the
    /// Network's prediction for a given input vector
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input vector
    pub fn predict(&mut self, inputs: &Array2<f64>, rng: Option<ThreadRng>) -> Array2<f64> {
        let mut output: Array2<f64> = inputs.to_owned();

        for layer in self.layers.iter_mut() {
            output = layer.feed_forward(&output, &rng);
        }

        output
    }
}

impl Serialize for Network {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        let mut s = serializer.serialize_struct("Network", 1)?;
        s.serialize_field("layers", &self.layers)?;

        s.end()
    }
}
