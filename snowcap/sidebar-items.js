initSidebarItems({"enum":[["Error","Main error type"]],"fn":[["optimize","Synthesize Configuration Updates while optimizing soft policiesThis is the main function to interact with the system. It uses the `OptimizerTRTA`."],["synthesize","Synthesize Configuration UpdatesThis is the main function to interact with the system. It uses the `StrategyTRTA`."],["synthesize_parallel","Synthesize Configuration Updates using multiple parallel threadsThis funciton spawns `N` `StrategyTRTA` threads, that search for a solution in parallel, using different random seeds.. The first solution found will be used, and all other threads will be killed."]],"mod":[["example_networks","Networks for testing"],["hard_policies","Hard PoliciesThis module contains all necessary structures and tools to generate hard policies as Linear Temporal Logic."],["modifier_ordering","ModifierOrderingThis module defines different orderings for `ConfigModifier`, and a trait as an interface."],["netsim","NetSimThis is a library for simulating specific network topologies and configuration."],["optimizers","OptimizerOptimizers try to solve the problem of reconfiguraiton, by always requiring the (hard) policies to hold, while trying to minimize the cost of soft policies. The following optimizers exist:"],["permutators","PermutatorsThis module contains all different iterators which iterate over all permutations. The iterators differ from each other by the order in which the permutations are yielded."],["soft_policies","Soft PoliciesSoft policies are expressed as cost functions, the smaller the result fo the cost functions, the better is the solution which is found."],["strategies","StrategiesThis module contains the source codes for different strategies for finding a valid update sequence. It contains the trait definition for `Strategy`, which `Snowcap` will use."],["topology_zoo","Import functions for importing topology zoo graphml files"]],"struct":[["Stopper","Stopper, to check when to stop, or to send the stop command"]]});