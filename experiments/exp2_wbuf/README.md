# Experiment configuration

Mgen scripts generates stable 1_048_576 bit/s UDP cross traffic on each of the bottleneck links
Additionally 800000 bit TCP bursts lasting 2 seconds are injected on average every 15 seconds
This adds an additional 213_333 bit/s cross traffic on average.

The available bandwidth should fluctuate around 1.8 Mbit/s, never above 2 Mbit, but can go below 1 Mbit/s

This will output on average 533 probe gap datapoints each burst.


- Each node outputs 524288 bit/s to each other node not under the same subnet
- Bottleneck capacity = `3 mbit/s`
- Capacity of direct outgoing links = `5 mbit/s`
- Default Available bandwidth = `2 mbit/s`
