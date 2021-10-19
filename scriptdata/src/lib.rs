mod document;

pub trait ScriptdataWriter {
    type Error;
    type Output;
    type ElementWriter: ElementWriter<Output=Self::Output>;

    fn scalar_document<I: Into<document::ScalarItem>>(&mut self, value: I) -> Result<Self::Output, Self::Error>;
    fn table_document(&mut self, meta: Option<&str>) -> Self::ElementWriter;
}

pub trait ElementWriter {
    type Error;
    type Output;

    fn scalar_entry<'s, K, I>(&mut self, key: K, value: I) -> Result<(), Self::Error>
    where K: Into<document::Key<'s>>, I: Into<document::ScalarItem>;
    
    fn begin_table<'s, K>(&mut self, key: K, meta: Option<&'s str>) -> Result<document::TableId, Self::Error>
    where K: Into<document::Key<'s>>;

    fn end_table(&mut self)-> Result<(), Self::Error>;

    fn finish(self) -> Result<Self::Output, Self::Error>;
}

pub trait ReopeningWriter: ElementWriter {
    fn reopen_table<'s, K>(&mut self, key: K, tid: document::TableId) -> Result<(), Self::Error>
    where K: Into<document::Key<'s>>;
}