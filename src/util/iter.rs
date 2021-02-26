
/// Like FlatMap, but more than one function.
///
/// Note that this clones the items the input gives us.
pub struct FlatMapChain<TIn, TInItem, TOut, TF1, TF2, TI1, TI2>
where
    TIn: Iterator<Item=TInItem>,
    TInItem: Clone,
    TF1: FnMut(TIn::Item) -> Option<TI1>,
    TF2: FnMut(TIn::Item) -> Option<TI2>,
    TI1: Iterator<Item=TOut>,
    TI2: Iterator<Item=TOut>
{
    source: TIn,
    func_one: TF1,
    func_two: TF2,
    iter_one: TI1,
    iter_two: TI2,
    state: FlatMapChainState
}

enum FlatMapChainState {
    Next,
    YieldOne,
    YieldTwo,
    Done
}

impl<TIn, TInItem, TOut, TF1, TF2, TI1, TI2> Iterator for FlatMapChain<TIn, TInItem, TOut, TF1, TF2, TI1, TI2>
where
    TIn: Iterator<Item=TInItem>,
    TInItem: Clone,
    TF1: FnMut(TIn::Item) -> Option<TI1>,
    TF2: FnMut(TIn::Item) -> Option<TI2>,
    TI1: Iterator<Item=TOut>,
    TI2: Iterator<Item=TOut>
{
    type Item = TOut;
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}