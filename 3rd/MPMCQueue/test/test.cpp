#include <gtest/gtest.h>

#include <MPMCQueue.hpp>
#include <atomic>
#include <thread>
#include <vector>

using namespace mpmc_queue;

TEST(MPMCQueueTest, BasicPushPop) {
  MPMCQueue<int, 4> queue;
  int val = 0;

  EXPECT_TRUE(queue.push(1));
  EXPECT_TRUE(queue.push(2));
  EXPECT_TRUE(queue.push(3));
  EXPECT_TRUE(queue.push(4));
  EXPECT_FALSE(queue.push(5));  // Full

  EXPECT_TRUE(queue.pop(val));
  EXPECT_EQ(val, 1);
  EXPECT_TRUE(queue.pop(val));
  EXPECT_EQ(val, 2);
  EXPECT_TRUE(queue.pop(val));
  EXPECT_EQ(val, 3);
  EXPECT_TRUE(queue.pop(val));
  EXPECT_EQ(val, 4);
  EXPECT_FALSE(queue.pop(val));  // Empty
}

TEST(MPMCQueueTest, MultiThreadedPushPop) {
  MPMCQueue<int, 1024> queue;
  std::atomic<int> sum{0};
  const int num_ops = 1000;
  const int num_threads = 4;

  auto producer = [&]() {
    for (int i = 0; i < num_ops; ++i) {
      while (!queue.push(1)) {
        std::this_thread::yield();
      }
    }
  };

  auto consumer = [&]() {
    int val;
    for (int i = 0; i < num_ops; ++i) {
      while (!queue.pop(val)) {
        std::this_thread::yield();
      }
      sum += val;
    }
  };

  std::vector<std::thread> producers;
  std::vector<std::thread> consumers;

  for (int i = 0; i < num_threads; ++i) {
    producers.emplace_back(producer);
    consumers.emplace_back(consumer);
  }

  for (auto& t : producers) t.join();
  for (auto& t : consumers) t.join();

  EXPECT_EQ(sum, num_ops * num_threads);
}

TEST(MPMCQueueTest, HeavyLoadStressTest) {
  MPMCQueue<int, 65536> queue;
  std::atomic<long long> sum{0};
  const int num_ops_per_thread = 100000;
  const int num_threads = 16;

  auto producer = [&](int /*id*/) {
    for (int i = 0; i < num_ops_per_thread; ++i) {
      while (!queue.push(1)) {
        std::this_thread::yield();
      }
    }
  };

  auto consumer = [&]() {
    int val;
    for (int i = 0; i < num_ops_per_thread; ++i) {
      while (!queue.pop(val)) {
        std::this_thread::yield();
      }
      sum += val;
    }
  };

  std::vector<std::thread> producers;
  std::vector<std::thread> consumers;

  for (int i = 0; i < num_threads; ++i) {
    producers.emplace_back(producer, i);
    consumers.emplace_back(consumer);
  }

  for (auto& t : producers) t.join();
  for (auto& t : consumers) t.join();

  EXPECT_EQ(sum, static_cast<long long>(num_ops_per_thread) * num_threads);
}

TEST(MPMCQueueTest, ManyProducersFewConsumers) {
  MPMCQueue<int, 1024> queue;
  std::atomic<long long> consumer_sum{0};
  const int num_producers = 12;
  const int num_consumers = 4;
  const int ops_per_producer = 10000;

  // Total items produced = num_producers * ops_per_producer
  // Each consumer must consume = (Total items) / num_consumers
  const int total_items = num_producers * ops_per_producer;
  const int ops_per_consumer = total_items / num_consumers;

  auto producer = [&]() {
    for (int i = 0; i < ops_per_producer; ++i) {
      while (!queue.push(1)) {
        std::this_thread::yield();
      }
    }
  };

  auto consumer = [&]() {
    int val;
    for (int i = 0; i < ops_per_consumer; ++i) {
      while (!queue.pop(val)) {
        std::this_thread::yield();
      }
      consumer_sum += val;
    }
  };

  std::vector<std::thread> producers;
  std::vector<std::thread> consumers;

  for (int i = 0; i < num_producers; ++i) producers.emplace_back(producer);
  for (int i = 0; i < num_consumers; ++i) consumers.emplace_back(consumer);

  for (auto& t : producers) t.join();
  for (auto& t : consumers) t.join();

  EXPECT_EQ(consumer_sum, total_items);
}

TEST(MPMCQueueTest, QueueFullEmptyStress) {
  MPMCQueue<int, 16> queue;
  const int iterations = 100000;

  auto producer = [&]() {
    for (int i = 0; i < iterations; ++i) {
      while (!queue.push(i)) {
        std::this_thread::yield();
      }
    }
  };

  auto consumer = [&]() {
    int val;
    int count = 0;
    // 4 producers * iterations
    while (count < iterations * 4) {
      if (queue.pop(val)) {
        count++;
      } else {
        std::this_thread::yield();
      }
    }
  };

  std::vector<std::thread> producers;
  for (int i = 0; i < 4; ++i) producers.emplace_back(producer);

  std::thread c(consumer);

  for (auto& p : producers) p.join();
  c.join();
}

TEST(MPMCQueueTest, FewProducersManyConsumers) {
  MPMCQueue<int, 1024> queue;
  std::atomic<long long> consumer_sum{0};
  const int num_producers = 4;
  const int num_consumers = 12;
  const int ops_per_consumer = 10000;

  // Total items needed = num_consumers * ops_per_consumer
  // Each producer must produce = (Total items) / num_producers
  const int total_items = num_consumers * ops_per_consumer;
  const int ops_per_producer = total_items / num_producers;

  auto producer = [&]() {
    for (int i = 0; i < ops_per_producer; ++i) {
      while (!queue.push(1)) {
        std::this_thread::yield();
      }
    }
  };

  auto consumer = [&]() {
    int val;
    for (int i = 0; i < ops_per_consumer; ++i) {
      while (!queue.pop(val)) {
        std::this_thread::yield();
      }
      consumer_sum += val;
    }
  };

  std::vector<std::thread> producers;
  std::vector<std::thread> consumers;

  for (int i = 0; i < num_producers; ++i) producers.emplace_back(producer);
  for (int i = 0; i < num_consumers; ++i) consumers.emplace_back(consumer);

  for (auto& t : producers) t.join();
  for (auto& t : consumers) t.join();

  EXPECT_EQ(consumer_sum, total_items);
}

TEST(MPMCQueueTest, EightProducersEightConsumers) {
  MPMCQueue<int, 4096> queue;
  std::atomic<long long> consumer_sum{0};
  const int num_producers = 8;
  const int num_consumers = 8;
  const int ops_per_producer = 50000;

  // Total items produced = num_producers * ops_per_producer
  // Each consumer must consume = (Total items) / num_consumers
  const int total_items = num_producers * ops_per_producer;
  const int ops_per_consumer = total_items / num_consumers;

  auto producer = [&]() {
    for (int i = 0; i < ops_per_producer; ++i) {
      while (!queue.push(1)) {
        std::this_thread::yield();
      }
    }
  };

  auto consumer = [&]() {
    int val;
    for (int i = 0; i < ops_per_consumer; ++i) {
      while (!queue.pop(val)) {
        std::this_thread::yield();
      }
      consumer_sum += val;
    }
  };

  std::vector<std::thread> producers;
  std::vector<std::thread> consumers;

  for (int i = 0; i < num_producers; ++i) producers.emplace_back(producer);
  for (int i = 0; i < num_consumers; ++i) consumers.emplace_back(consumer);

  for (auto& t : producers) t.join();
  for (auto& t : consumers) t.join();

  EXPECT_EQ(consumer_sum, total_items);
}
