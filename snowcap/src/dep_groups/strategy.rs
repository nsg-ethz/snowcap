// Snowcap: Synthesizing Network-Wide Configuration Updates
// Copyright (C) 2021  Tibor Schneider
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program; if not, write to the Free Software Foundation, Inc.,
// 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.

//! # DepGroupsStrategy
//!
//! This module contains the implementation of the `DepGroupStrategy`.

use super::utils;
use crate::hard_policies::{HardPolicy, PolicyError};
use crate::modifier_ordering::SimpleOrdering;
use crate::netsim::config::ConfigModifier;
use crate::netsim::Network;
use crate::permutators::{Permutator, PermutatorItem, RandomTreePermutator};
use crate::strategies::{GroupStrategy, PushBackTreeStrategy, Strategy};
use crate::{Error, Stopper};

use log::*;
use rand::prelude::*;
use std::marker::PhantomData;
use std::time::{Duration, SystemTime};

/// # The Dependency Groups Builder Strategy
/// This strategy tries to build groups which are dependent of oneanother, and tries to solve these
/// independently of all other modifications. The strategy should work really good when there are
/// many different, smaller groups of dependent modifications, which are not dependent between each
/// other.
///
/// ## Properties
///
/// This strategy benefits from problems with an *immediate effect*, since it can massively reduce
/// the search space if a problem is detected with a relatively small problem. In addition, this
/// strategy is able do deal with many smaller dependencies (with *no immediate effect* and a
/// *sparse solution*), because it scales with `O(g^4)` for the number of dependency groups. But
/// if the groups become larger, it scales with `O(n!)`.
///
/// ## Type Arguments
/// - `S` is a [`GroupStrategy`](crate::strategies::GroupStrategy), used to solve a smaller problem
///   with the group information learned before.
/// - `P` is a [`Permutator<usize>`](crate::permutators::Permutator), used to generate all
///   permutations of the groups. As soon as a new group is formed, the permutator is reset.
///
/// ## Overview of the strategy
///
/// * Initially, we start by having all modifiers in their own group.
///
/// * Repeat the following procedure, until we have found a valid solution:
///
///   1. *Choose ordering*: Choose a random ordering of the groups, but respect the inter-group
///      ordering that was determined in step 3.2.
///
///   2. *Check ordering*: If the ordering works, then the algorithm ends successfully. If the
///      ordering has a problem at group `gi`, then continue at step 3.
///
///   3. *Identify minimal problem*: This is a procedure with multiple phases
///
///      1. *Reduction phase*: Go through all groups `gj` in `g1, g2, ..., g(i-1)`, and temporatily
///         remove it from the sequence. Then, retry to execute the sequence. If the resulting
///         errors are in any way different, we keep the group `gj` in the ordering. Else, we assume
///         that this group `gj` is independent of the problem, and we remove it from the ordering.
///
///      2. *Solving phase*: Try to find a solution to the problem, using a different exhaustive
///         strategy. Here, we try to retain all orerings of the sub-groups to speed up the process
///         If we have found a valid sequence in which the problem can be found, then we declare the
///         remaining modifiers as a group, and store it in the solvable sequence. Continue at step
///         1. If not, then go to the expansion phase at step 3.3.
///
///      3. *Expansion phase*: If step 3.2 did not result in a solved group, then we expand the
///         group uner consideration: Continuing form step 3.2, we apply single groups from after
///         the last one which failed at step 2, and we monitor the changes of errors. If the errors
///         have changed, then we add this group to the current group under consideration and go
///         back to step 3.2 to solve the problem. If there exists no modifier group which will
///         change the errors, we clare this try as failed, we learn nothing from it and continue at
///         step 1, with a guard that after 10 successive iterations, not finding anything, we abort
///         the strategy.
///
/// ## Some Details
///
/// - In step 3.1, we don't only need to remove a modifier to see wether it changes the output, but
///   we also need to move it to the front. Ideally, we would move this modifier to every position,
///   but we could argue that this is not necessary due to the order in which we look at the groups.
///   TODO write down a proof or give intuition why this is sufficient.
///
/// - In step 3.1, when the we have found a smaller problem inside the current problem due to the
///   error being thrown at a different group, we cannot just use recurrsion to further reduce
///   the problem. This is because, when we use the ordering with the group at the position it was
///   before, the smaller problem is solvable (because the problem occurred at a different
///   position). But if we would just remove it and do the recurrsion, it would never solve the
///   problem later in step 3.2 or 3.3, because there, we add remaining groups, which come after the
///   problem group. Thus, we need to remove it for the recurrsion and then add it back after it is
///   finished. However, we need to add it back *only* when the last element of the ordering before
///   and after the recurrsion are still the same.
///
/// - In step 3.1, when removing several groups to get the minimal problem, we really want a minimal
///   problem in terms of groups. If, after removing a modifier, the group changes where the error
///   appears, we remove all groups after that problematic group. Then, we rerun step 3.1 from the
///   beginning, because we are now actually searching for a different problem, and groups, which
///   are identified before, might no longer be part of the problem.
///
/// - In step 3.3, it might happen that adding the new modifier at the beginning of the problem
///   doesn't change the output, even though it actually is part of the problem. To reduce the
///   probability of such a case happening, we also try to put the modifier at the end of the
///   sequence, right before the problematic modifier. (*TODO*: We should find an example where
///   putting the modifier at the start or at the end does not change the result, but adding it in
///   the middle somewhere does.)
///
/// - During step 3.3, we do not allow the problem to be shifted to the newly added group, if that
///   one cannot be applied at the beginning! However, it might happend that the group solves the
///   problem if applied at the end, right before the problematic group.
///
/// - When we compare for the errors to change, we cannot simply require that we have solved some
///   errors, and that the old errors must be a superset of the new errors. As an example, while
///   reducing the problem, noticing that the new errors introduce new policy errors, we would make
///   the problem actually bigger by declaring this modifier independent and removing it from the
///   groups. Since we already check if the position of the error has changed, any introduction of
///   new errors means that this modifier belongs to the group.
///
/// - As we have shown in the proof of lemma 3 (below), it is necessary that we go back to the
///   reduction phase, when we the group containing the problematic modifier has changed during
///   expansion phase. **TODO** Implement this.
///
/// ## Future Considerations
/// - In step 3.3, we might want to check if adding the modifier somewhere in between does change
///   the outcome.
/// - As long as we don't check every single position, this strategy is not exhaustive!
/// - In step 3.3, when we notice that a modifier cannot be applied at the beginning, we should
///   actually check if it changes something when adding it later. The reason is that he might solve
///   the problem when applied later in the group.
///
/// ## Finding Dependencies is hard
///
/// *If you find too large dependencies, solving them might take too long*: Take, as an example, the
/// [`DifficultGadgetComplete`](crate::example_networks::DifficultGadgetComplete). If we detect a
/// dependency group like this, and try to solve it, it might take $O(n!)$, even though not all
/// modifiers are actually dependent, and they can be rearranged however you like. Thus, it is
/// important that the learned groups are as small as possible.
///
/// *Information from one dependency group cannot be used to solve another dependency group!* Assume
/// that we have a group, for which there exists only one single valid ordering. As an example, take
/// $m_1$, $m_2$, $m_3$, where this ordering is valid. Further, assume that for the chosen ordering,
/// inserting $m_4$ at any position results in an invalid ordering. The only valid ordering with
/// $m_4$ is $m_3$, $m_4$, $m_2$, $m_1$. This means, that we need to solve the problem again with
/// the larger solution, and we cannot use the information, that we have already obtained by solving
/// the smaller problem. The [`BipartiteGadget`](crate::example_networks::BipartiteGadget) is a good
/// example where this happens. A dependency group is learned, but it is not complete, and the
/// solution is invalid when the other modifiers are added. This group is then expanded, and solved
/// multiple times, always increasing the size.
///
/// *Combining two dependency groups might not reult in another dependency group!* Assume that we
/// have two dependencies: $m_1$ must go before $m_2$, and $m_3$ must go before $m_4$. Further,
/// assume we notice that performing $m_1$ before $m_3$ breaks the dependency between $m_3$ and
/// $m_4$, and that now $m_4$ can be inserted into any arbitrary position, if $m_1$ is before $m_3.
/// This shows, that having learned two dependency groups, the combination of them might not be a
/// strict dependency group.
///
/// ### How this strategy deals with these problems
///
/// The strategy is very conservative, when adding new modifiers to a group. We always extend the
/// group, in which the problematic modifier resides. Because if a modifier causes the problem, it
/// must also be a part of the new dependency group. Any other modifier, that survives the
/// reduction phase satisfies one of the following conditions:
///
/// 1. Removing it from the ordering solves the problem $\rightarrow$ It changes the outcome of the
///    group.
/// 2. Removing it from the ordering changes the problematic modifier. In this case, we reduce the
///    problem by only considering this smaller problem. Then, we recursively restart the reduction
///    phase, to continue removing all modifiers that no longer are necessary for the group.
/// 3. Removing it from the ordering changes the problems that occur. The chance for this modifier
///    being a part of the group is large. There might be cases, in which a modifier never causes
///    the problem, but only changes how it manifests itself as problems in the network. However,
///    it is very hard to see this withouth trying all possible orderings. Thus, adding it to the
///    group is a reasonable thing to do.
///
/// Thus, we only learn groups, which are somehow dependent. However, assume that we have already
/// learned the two incomplete dependency groups $A$ and $B$. Now, we notice that $A$ must happen
/// before $B$. Combining $A$ and $B$ might result in a modifier no longer being a dependency in the
/// sense that there exists no ordering, in which rearranging that modifier will change the outcome
/// of the ordering (even if this was previously the case in its smaller dependency group $A$ or
/// $B$). But still, one might say that it is remains dependent of $A \cup B$, because either $A$ or
/// $B$ was dependent on it.
///
/// ### Worst Cases
///
/// Consider the [`BipartiteGadget`](crate::example_networks::BipartiteGadget). There exists a case,
/// where we have found an incomplete dependency group, that contains the following
/// modifiers:
///
/// - All sessions from `tI` to `xI`
/// - All sessions from `rI` to `bI`, except one at `I = 1` will be removed
///
/// In this case, every ordering where the session from `t0` to `x0` is established first, will be
/// valid (which is what we actually want to find out), but also every ordering, where the session
/// `t1` to `x1` is added first (which is wrong when considering all modifiers). Assume, that the
/// ordering learned is one where the session `t1` to `x1` is added first, and the session from `t0`
/// to `x0` is added later. Then, in a subsequent iteration, the algorithm notices that the removal
/// of session `r1` to `b1` also is a part of the problem. Now, both orderings are invalid, where
/// the new modifier is added in the beginning or at the end, and thus, we need to recompute the
/// entire problem, and brute-force it.
///
/// This problem can be made even worse, when considering that there is no valid solution, and we
/// need to expand the problem multiple times, without finding any solution. In this case, finding
/// a solution takes:
///
/// $$\sum_{i=0}^n O(i!) = O(n!)$$
///
/// # Proof that no learned dependency group is too large
///
/// *Theorem 1*: The `DepGroupsStrategy` does only learn weak dependency groups.
///
/// For the proof, we interchange single modifiers and entire modifier groups, for which we already
/// know a valid ordering, and which we always keep together in this valid ordering. We first need
/// to define some terms before we can proof the theorem:
///
/// - **Configuration**: A configuration $C$ is a tuple $(\mathcal{G}, \mathcal{C}, \mathcal{S})$,
///   where $\mathcal{G}$ denotes the network topology, $\mathcal{C}$ denotes a network wide
///   configuration and $\mathcal{S}$ denotes a network state.
///
/// - **Configuration Modification**: A configuration modification $m$ is a single modification of the
///   network-wide configuration $\mathcal{C}$ Based on a configuration $C$, applying a modification
///   $m$ is expressed as $C \cdot m = C'$, which results in a different configuration $C'$. Notice,
///   that $C' = (\mathcal{G}, \mathcal{C}', \mathcal{S}')$. Even though this operation seems to be
///   cumulative in most cases, this is not always the case (see the
///   [Unstable Gadget](crate::example_networks::DifficultGadgetMinimal)).
///
/// - **Policies**: The Policies $\mathcal{P}$ is a set of hard-policies that need to be satisfied for
///   some state $\mathcal{S}$ of configuration $C$. This fact is denoted by $C \vdash \mathcal{P}$.
///
/// - **Valid Ordering**: The ordering $o$ of a set of modifiers $D = \{m_1, m_2, \ldots, m_n \}$ on
///   configuration $C$ under policy $\mathcal{P}$ is valid, if and only if $\forall\ x \leq n: C
///   \cdot m_{o(1)} \cdot m_{o(2)} \cdot \ldots \cdot m_{o(x)} \vdash \mathcal{P}$.
///
/// - **Similar Ordering**: A modifier ordering $o'$ of modifiers $M = \{m_1, \ldots, m_n\}$ is
///   similar to $o$ with respect to a subgroup of modifiers $M' \subseteq M$, $|M'| > 0$, if this
///   subgroup $M'$ is moved inside of $o$. During this transformation, the relative ordering of $x$
///   and $y$ must be preserved for all pairs $x, y \in M'$ and for all pairs $x, y \in M \setminus
///   M'$. The relative ordering of $x$ and $y$ must not be preserved if and only if $x \in M'$ and
///   $x \in M \setminus M'$.
///
/// - **Problematic Modifier** Given a set of modifiers $M$, an invalid ordering $o$, a configuration
///   $C$ and a policy $\mathcal{P}$, let $x \leq n$ be the smallest number, for which $C \cdot
///   m_{o(1)} \cdot \ldots \cdot m_{o(x)}$ does not satisfy $\mathcal{P}$. Then, $m_x$ is called
///   the problematic modifier of $M$ with $o$ on $C$ under $\mathcal{P}$.
///
/// - **Critical Group**: A modifier group $M' \subset M$ is critical for the set of modifiers $M =
///   (m_1, \ldots, m_n)$ on $C$ under $\mathcal{P}$, if at least one of the following conditions
///   apply:
///
///   1. There exaists a valid ordering $o$, and a similar ordering $o'$ with respect to $M'$ which
///      is not valid. In other words, there exists a valid ordering where moving the group $M'$
///      around will result in an invalid ordering somewhere.
///   2. There exists an invalid ordering $o$, and a similar ordering $o'$ with respect to $M'$,
///      where the resulting error of $o$ and $o'$  is different, or the problematic modifier has
///      changed.
///
/// - **Dependency Group**: A set of modifiers $D = \{ m_1, m_2, \ldots, m_n \}$ is called a
///   dependency group on configuration $C$ under policy $\mathcal{P}$ if the following holds:
///   1. There exists a *valid ordering* $o$ for $D$ on $C$ under $\mathcal{P}$.
///   2. Every subgroup $M' \subset D$, with $|M'| > 0$ is *critical* for $D$ on $C$ under
///      $\mathcal{P}$.
///
/// - **Weak Dependency Group**: A set of modifiers $D' \subseteq D$ is a weak dependency group if it
///   is a subset of a dependency group $D$.
///
/// *Observation 1*: Any ordering $\tilde{o}$ is valid, if it is the beginning of a valid ordering
/// $o$.
///
/// *Observation 2*: If any ordering $o$ is invalid, any other ordering $\tilde{o}$ is also invalid
/// if it starts with the same sequence of modifiers, up to and including the problematic modifier.
///
/// *Observation 3*: Let $A \subset M$ and $B \subset M$ be two subsets of $M$, which are disjoint
/// ($A \cap B = \emptyset$). Let $o$ be an ordering, and $o'$ be a similar ordering of $o$ with
/// respect to $A$. Then, we construct an ordering $o_B$, similar to $o$, by moving $B$ to the
/// beginning or to the end, and we construct $o'_B$, similar to $o'$, by moving $B$ to the
/// beginning or the end. Then, $o_B$ and $o'_B$ are still similar with respect to $A$.
///
/// *Lemma 1*: Let $A$ be a critical group to the set of modifiers $M$, and let $B$ be a
/// non-critical group to $M$. Then, $A$ is also critical to $M \setminus B$.
///
/// *Proof of Lemma 1*: There are two cases, why $A$ is critical to $M$. We need to proof the fact
/// for the following three cases:
///
/// 1. There exists a valid ordering $o$ of $M$, and an invalid, and similar ordering $o'$ to $o$
///    with respect to $A$. In this case, rearranging $B$ in $o$ does not change the fact that $o$
///    is valid, and rearranging $B$ in $o'$ does not make the ordering valid. Also, in both $o$ and
///    $o'$, the ordering of $B$ is valid (which can be quickly shown by moving $B$ to the beginning
///    of $o$, and using the Observation 1). Thus, we generate $o_B$ by moving $B$ to the end of
///    $o$, and we generate $o'_B$ by moving $B$ to the end of $o'$. Notice, that no modifier in $B$
///    can be the problematic modifier of $o'_B$, since (a) the relative ordering of $B$ is valid,
///    and (b) rearranging $B$ cannot change the problematic modifier. Thus, we have a valid
///    ordering $o_B$, and an invalid, similar ordering $o'_B$ with respect to $A$, which shows that
///    $A$ is still critical to $M \setminus B$.
/// 2. There exists an invalid ordering $o$ for $M$, and an invalid, and similar ordering $o'$ to
///    $o$ with respect to $A$, which has a different error. Similarly to the case 1, we can
///    generate similar orderings to $o$ and $o'$ by moving $B$ to the end. This does not change
///    the error of $o$ and $o'$. Thus, they still have a different error, and they still are
///    similar (see Observation 3), which shows that $A$ is still critical to $M \setminus B$.
/// 3. There exists an invalid ordering $o$ for $M$, and an invalid, and similar ordering $o'$ to
///    $o$ with respect to $A$, which has a different problematic modifier. Notice, that the new
///    problematic modifier cannot be in $B$ (by applying the same proof as in case 1). Again, by
///    applying the same method as in case 1, we can move the modifier $B$ to the end without
///    changing the problematic modifier. Thus, the resulting ordering still has two different
///    problematic modifiers, and hence, $A$ is still critical to $M \setminus B$.
///
/// <p style=text-align:right;">$\square$</p>
///
/// *Lemma 2*: Let $A$ and $B$ be two dependency grups. If $A$ can be applied before $B$, but
/// $B$ cannot be applied before $A$, then $A \cup B$ forms a dependency group.
///
/// *Proof of Lemma 2*: **TODO** This is not the case based on the current definition of a
/// dependency group. But on a higher level, it should definately be the case. Thus, we might
/// need to change the definition, such that this is included. For the following, we will just
/// assume that this is true.
///
/// *Lemma 3*: After the reduction phase of the algorithm, the resulting group is a weak dependency
/// group.
///
/// *Proof of Lemma 3*: In the following, we will call $M_i$ the group under consideration at
/// iteration $i$. We will show that at each iteration of the reduction phase, if a group $A$
/// survives, it is critical to $M_i$. This suffices, because we know from Lemma 1, that if we
/// remove a non-critical group form $M_i$, the group $A$ is still critical to $M_{i+1}$. And if
/// we remove a critical group to $M_i$, then the resulting group $M_{i+1}$ is a weak dependency
/// group. By recursively applying this fact, we can see that what remains after all $k$ iterations
/// is a weak dependency group.
///
/// Now, we will show that at each iteration of the reduction phase, if group $A$ survives, it is
/// critical to $M_i$. There are three cases in which $A$ remains:
///
/// 1. Removing $A$ solves the problem. Let $o$ be the ordering before removing $A$, which we know
///    is invalid. Let $o_A$ be the ordering where $A$ is moved to the back of $M_i$. Since removing
///    $A$ solves the problem, we know that either $o_A$ is valid, or in $o_A$, the problematic
///    modifier is in $A$. If $o_A$ is valid, then we know that $A$ is critical to $M_i$. If the
///    problematic modifier is in $A$, then we also know that $A$ is critical to $M_i$, because
///    previously, the problematic modifier was not in $A$ (else, we would not try to remove $A$
///    from $M_i$ during reduction phase).
/// 2. Removing $A$ changes the problem. Again, let $o$ be the ordering before removing $A$, which
///    we know is invalid. Let $o_A$ be the ordering where $A$ is moved to the back of $M_i$. With
///    Observation 2, we can see that $o_A$ produces the same problem at the same problematic
///    modifier as $o$, with $A$ removed. Thus, $A$ is critical to $M_i$.
/// 3. Removing $A$ changes the problematic modifier. By applying the same argument as in case 2,
///    $A$ is critical to $M_i$. However, notice that we recursively restart the reduction phase,
///    and recheck all previously determined groups if they are still critical.
///
/// <p style=text-align:right;">$\square$</p>
///
/// *Lemma 4*: During expansion phase, starting form a weak dependency group $M_1$, we only add
/// critical groups.
///
/// *Proof of Lemma 4*: Again, there are several different cases in which a modifier group $A$ is
/// added to the current group $M_i$ at iteration $i$.
///
/// 1. Inserting the group $A$ either at tbe beginning or at the end of the ordering, right before
///    the current problematic group, does solve the problem. Let $o$ be the ordering before
///    inserting $A$ into the ordering, and $o'$ the ordering where $A$ is inserted. Notice, that we
///    can extend $o$ by applying $A$ at the end. This does not change the problem, since the
///    problematic modifier happens before $A$ is applied. Thus, $o'$ and $o$ are similar with
///    respect to $A$, and thus, $A$ is critical to $M_i$.
/// 2. Inserting the group $A$ either at the beginning or at the end of the ordering, right before
///    the current problematic group, does change the error. Following the same argumentation of
///    case 1, we can see that $A$ is critical to $M_i$.
/// 3. Inserting the group $A$ either at the beginning or at the end of the ordering, right before
///    the current problematic group, does change the problematic modifier. Same as for case 1 and
///    2, we know that $A$ is critical to $M_i$. However, since we now reduce $M_i$ to only include
///    all groups up to the group containing the problematic modifier, we need to rerun the
///    reduction phase, since the resulting group might no longer be a weak dependency group.
///    **TODO** implement this in the code.
///
/// Based on Lemma 2, we know that the expanded group $A \cup M_i = M_{i+1}$ is also a weak
/// dependency group. This proves this lemma.
///
/// <p style=text-align:right;">$\square$</p>
///
/// *Proof of Theorem 1*: By using Lemma 3 and 4, we can see that every group, which is added to the
/// set of groups has both a valid ordering, and is a (weak) dependency group.
///
/// <p style=text-align:right;">$\square$</p>
///
/// ## Reason why this strategy is not exhaustive
///
/// Let $M = \lbrace m_1, m_2, m_3, m_4 \rbrace$. Assume that there are the following two
/// dependencies:
///
/// 1. $(m_1, m_2)$: This is a dependency, which has an immediate effect if $m_2$ is applied before
///    $m_1$.
/// 2. $(m_1, m_3, m_2, m_4)$: This dependency has no immediate effect. If $m_4$ is applied, and all
///    other modifiers are not applied before in the correct order, then the policy is no longer
///    satisfied, with always the exact same reason.
///
/// In the following, we argue that there are cases in which the `DepGroupsStrategy` cannot solve the
/// problem. for this, we need to consider two points:
///
///  1. Before we have learned anything, we have the following cases:
///     1. Both $m_1$ and $m_2$ are before $m_4$. In this case, no matter the ordering or $m_1$ and
///        $m_2$, we will learn that $m_2$ is dependent on $m_1$ (during reduction phase if $m_1$
///        is before $m_2$, or during expansion phase if $m_2$ is before $m_1$).
///     2. $m_2$ is before $m_4$. In this case, the same happens as in case 1.
///     3. $m_1$ is before $m_4$. In this case, the same happens as in case 4.
///     4. $m_4$ is before both $m_1$ and $m_2$. In this case, it fails at $m_4$, and the reduciton
///        phase will result in $m_4$ being the only modifier left. Then, the expansion phase will
///        have no effect.
/// 2. When we have learned dependency $(m_1, m_2)$, we cannot learn anything anymore. Every
///    reduction phase will result in $m_4$ begin the only modifier left. Then, the expansion
///    phase will not find anything, because the problem is always the same if not in the single
///    correct ordering.
///
/// The next question is, what is the probability of `DepGroupsStrategy` finding a valid solution.
/// For this, we need to count the number of possible orderings, in which the algorithm will find
/// dependency $(m_1, m_2)$, which means the algorithm will fail, and the number of orderings in
/// which the algorithm finds the valid solution. Out of the possible 24 different orderings, only 1
/// will result in the algorithm succeeding. However, there are 11 orderings in which the algorithm
/// will learn the dependency $(m_1, m_2)$. All other 12 cases we can ignore, because these
/// orderings will just cause the algorithm to choose a different ordering and try again. This leads
/// to a 9% probability of success.
///
/// The question is now, can we find an example where this case happens?
///
/// ## Test Case Reduction / Delta Debugging
///
/// In test Case Reduction, we try to reduce a problem in a computer program to the minimal set,
/// which reproduces the same error, in order for the programmer to see the problem more easily and
/// fix the bug, without being overwhelmed by information.
///
/// We actually do a similar thing in this strategy, by trying to find the minimal set of
/// modifications that cause a certain problem, and then, we try to fix it. The following are the
/// similarities, i.e., how we can reformulate our problem to make it more similar to Test Case
/// Reduction:
///
/// - Our oracle is the network simulator. Obviously, this oracle is not perfect, because devices in
///   the real world might behave differently. However, when we just consider the reduction phase,
///   and how we reduce the problem to a dependency group, we may argue that the oracle is perfect,
///   since it is both the oracle and the "real world" (only for this case).
/// - As in Case Reduction, we wish to keep only those modifiers where we know are part of the
///   dependency group. This is similar to Case Reduction, where an example is minimized, in order
///   to be better understood.
///
/// However, there are some key differences:
///
/// - In our case, the ordering of the modifiers can be chosen, and does certainly matter to the
///   output of the oracle. Thus, several ideas cannot be directly applied, because the problem is
///   not agnostic to the ordering. On way in which this fact materializes itself is that we cannot
///   do canonicalization (bringing the ordering into canonical form), because changing the order
///   will most likely change the result.
/// - Our oracle is better than for case reduction. The oracle tells us which modifier caused the
///   problem and what part of the network caused the error. We may use this insight to improve
///   our algorithms.
/// - In Test Case Reduction, one usually needs an ordering of the reduced test cases. Usually,
///   you would take something like [Shortlex order](https://en.wikipedia.org/wiki/Shortlex_order),
///   which prefers short test cases over long ones. In our case, however, one might argue that we
///   need to know about all these reduced orerings. If they are disjoint, then we need both of
///   them, and if they are not disjoint, then we probably need to merge them.
/// - [Delta Debugging](https://en.wikipedia.org/wiki/Delta_debugging) cannot be applied directly
///   due to the fact that the ordering matters. Rearranging can completely change the outcome.
///   Only checking the second part also makes no sense, if we never reach this point, and if the
///   modifications are applied on a different state of the network.
/// - The input is not a series of bytes, but a set of modifiers. These modifiers are (for now)
///   constant, and cannot be changed by the procedure. Also, our problem is different from the one
///   checking parsers (as demonstrated [here](https://www.fuzzingbook.org/html/Reducer.html)). For
///   parsers, the reduction passes never change the ordering of the input, but only remove certain
///   parts of the input.
///
/// Compared to some [notes](https://www.drmaciver.com/2019/01/notes-on-test-case-reduction/) on
/// test case reduction, we may be able to improve the algorithm by utilizing the following points:
///
/// 1. **Cache oracle results** using bloom filters. This may be necessary to do if simulating the
///    network becomes expensive. We might store different orderings of modifiers, which we know
///    cause problems, and where these problems occur, thus reducing the total amount of simulation
///    time. Since we cannot reduce an ordering into canonical form, and store it in this way into
///    the bloom filter, caching might not yield a big benefit in our case, especially since we use
///    permutators to never try the same ordering multiple times.
///
/// 2. Organize the code into **Reduction Passes**: One reduction pass is a function, which changes
///    a small thing of the current modifier ordering, like removing one, or reordering it.
///    Currently, this is somewhat distributed around the code. One reduction pass may invoke the
///    oracle a multiple times. A fix-point is reached if no change is possible in this reduction
///    pass. We need to ask the following questions:
///    - What reductions to try when the pass fails to reduce the current ordering?
///    - What reductions to try when the pass succeeds in reducing the current ordering?
///    - In what order wo we try to perform reductions?
///
/// 3. One might argue, that we can **use** the idea **directly**: Starting from an invalid ordering
///    from the beginning, we might want to reduce it just to the minimal set of modifiers. During
///    the reduction phase, we might want to try to find the smallest set of already learned groups,
///    that cause the problem. As soon as we have found a minimal problem, then we can try to solve
///    it by reordering, or by extending it with additional modifiers.
///
///    We can assume, that the algorithm will first just find those modifiers, which cannot be
///    applied by their own. But this is ok, since we wish to keep the dependencies as small as
///    possible. However, it might get more difficult to extend the problem again in order to find
///    a valid ordering.
///
///    Thus, this change would only result in a more aggressive reduction phase, which might also
///    find completely different problems (but if finds problems).
///
/// 4. Perform the reduction in a **Random Order**: As suggested by the
///    [notes](https://www.drmaciver.com/2019/01/notes-on-test-case-reduction/), performing the
///    reduction in a random order is often a good idea. Also, one should try to change more than
///    just one thing during the reduction phase, to reduce the total running time from $O(n)$ down
///    to $O(\log n)$.
///
/// Obviously, we can replace our reduction phase with proper methods from test case reduction.
/// However, early experiments have shown that the reduction only takes negligible time, compared to
/// finding a valid solution for a group.
pub struct DepGroupsStrategy<
    S = PushBackTreeStrategy<SimpleOrdering>,
    P = RandomTreePermutator<usize>,
> where
    S: Strategy + GroupStrategy,
    P: Permutator<usize> + Iterator,
    P::Item: PermutatorItem<usize>,
{
    net: Network,
    groups: Vec<Vec<ConfigModifier>>,
    permutator: P,
    hard_policy: HardPolicy,
    rng: ThreadRng,
    stop_time: Option<SystemTime>,
    max_group_solve_time: Option<Duration>,
    strategy_phantom: PhantomData<S>,
    #[cfg(feature = "count-states")]
    num_states: usize,
}

impl<S, P> Strategy for DepGroupsStrategy<S, P>
where
    S: Strategy + GroupStrategy,
    P: Permutator<usize> + Iterator,
    P::Item: PermutatorItem<usize>,
{
    fn new(
        mut net: Network,
        modifiers: Vec<ConfigModifier>,
        mut hard_policy: HardPolicy,
        time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error> {
        let num_modifiers = modifiers.len();
        let mut groups: Vec<Vec<ConfigModifier>> = Vec::with_capacity(modifiers.len());
        for modifier in modifiers {
            groups.push(vec![modifier]);
        }
        let permutator = P::new((0..groups.len()).collect());
        let mut fw_state = net.get_forwarding_state();
        hard_policy.set_num_mods_if_none(num_modifiers);
        hard_policy.step(&mut net, &mut fw_state)?;
        if !hard_policy.check() {
            return Err(Error::InvalidInitialState);
        }
        let max_group_solve_time: Option<Duration> =
            time_budget.as_ref().map(|dur| *dur / super::TIME_FRACTION);
        let stop_time: Option<SystemTime> = time_budget.map(|dur| SystemTime::now() + dur);
        Ok(Box::new(Self {
            net,
            groups,
            permutator,
            hard_policy,
            rng: rand::thread_rng(),
            stop_time,
            max_group_solve_time,
            strategy_phantom: PhantomData,
            #[cfg(feature = "count-states")]
            num_states: 0,
        }))
    }

    fn work(&mut self, mut abort: Stopper) -> Result<Vec<ConfigModifier>, Error> {
        'main_loop: loop {
            // check for iter overflow
            if self.stop_time.as_ref().map(|time| time.elapsed().is_ok()).unwrap_or(false) {
                // time budget is used up!
                error!("Time budget is used up! No solution was found yet!");
                return Err(Error::Timeout);
            }

            // check for abort criteria
            if abort.try_is_stop().unwrap_or(false) {
                info!("Operation was aborted!");
                return Err(Error::Abort);
            }

            // .--------.
            // | Step 1 | Choose random ordering
            // '--------'
            let ordering = match self.permutator.next() {
                Some(o) => o,
                None => {
                    error!("Strategy was not able to solve the problem!");
                    return Err(Error::NoSafeOrdering);
                }
            }
            .as_patches();

            debug!(
                "ordering of groups:\n{}",
                utils::fmt_group_ord(&self.groups, &ordering, &self.net),
            );

            // .--------.
            // | Step 2 | Check Ordering
            // '--------'
            let (problem_group_pos, errors) = match utils::check_group_ordering(
                self.net.clone(),
                &self.groups,
                &self.hard_policy,
                &ordering,
                #[cfg(feature = "count-states")]
                &mut self.num_states,
            ) {
                Ok(_) => {
                    // print the resulting groups
                    info!(
                        "Resulting groups in the respective order:\n{}",
                        utils::fmt_group_ord(&self.groups, &ordering, &self.net)
                    );
                    return Ok(utils::finalize_ordering(&self.groups, &ordering));
                }
                Err((_, i, Some(hp))) => (i, hp.get_watch_errors()),
                Err((_, i, None)) => (i, (Vec::new(), vec![Some(PolicyError::NoConvergence)])),
            };

            // .--------.
            // | Step 3 | Find dependencies
            // '--------'
            match utils::find_dependency::<S>(
                &self.net,
                &self.groups,
                &self.hard_policy,
                &ordering,
                errors,
                self.stop_time,
                self.max_group_solve_time,
                abort.clone(),
                #[cfg(feature = "count-states")]
                &mut self.num_states,
            ) {
                Some((new_group, old_groups)) => {
                    info!("Found a new dependency group!");
                    // add the new ordering to the known groups
                    utils::add_minimal_ordering_as_new_gorup(
                        &mut self.groups,
                        old_groups,
                        Some(new_group),
                    );

                    // prepare a new permutator for the next iteration
                    let mut group_idx: Vec<usize> = (0..self.groups.len()).collect();
                    group_idx.shuffle(&mut self.rng);
                    self.permutator = P::new(group_idx);

                    continue 'main_loop;
                }
                None => {
                    // Unable to extend the running group! Declare this try as failed and try
                    // again. tell the permutator that we have failed at the position
                    info!("Could not find a new dependency group!");
                    self.permutator.fail_pos(problem_group_pos);
                    // continue with the permutation
                    continue 'main_loop;
                }
            }
        }
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.num_states
    }
}
