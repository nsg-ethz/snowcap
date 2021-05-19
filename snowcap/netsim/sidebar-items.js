initSidebarItems({"enum":[["ConfigError","Configuration Error"],["DeviceError","Router Errors"],["NetworkDevice","Network Device (similar to `Option`)Enumerates all possible network devices. This struct behaves similar to an `Option`, but it knows two different `Some` values, the `InternalRouter` and the `ExternalRouter`. Thus, it knows three different `unwrap` functions, the `unwrap_internal`, `unwrap_external` and `unwrap_none` function, as well as `internal_or` and `external_or`."],["NetworkError","Network Errors"]],"mod":[["bgp","Module containing definitions for BGP"],["config","Network ConfigurationThis module represents the network configuration. There are several different structs in this module. Here is an overview:"],["external_router","External RouterThe external router representa a router located in a different AS, not controlled by the network operators."],["printer","Helper (printer) functions for the NetworkModule containing helper functions to get formatted strings and print information about the network."],["route_map","Route-MapsThis module contains the necessary structures to build route maps for internal BGP routers."],["router","Module defining an internal router with BGP functionality."]],"struct":[["AsId","AS Number"],["ForwardingState","Forwarding StateThis is a structure containing the entire forwarding state. It provides helper functions for quering the state to get routes, and other information."],["Network","Network structThe struct contains all information about the underlying physical network (Links), a manages all (both internal and external) routers, and handles all events between them. Configuration is applied on the network itself, treated as network-wide configuration."],["Prefix","IP Prefix (simple representation)"]],"type":[["IgpNetwork","IGP Network graph"],["LinkWeight","Link Weight for the IGP graph"],["RouterId","Router Identification (and index into the graph)"]]});