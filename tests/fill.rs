//! What a search's result left unfilled (docs/runner.md, 「The fill」): the cells read `unchecked`, the product
//! rows naming no page and a product result with no picture, read off product results alone.

use research_server::fill::gaps;

const TABLE: &str = "| Product | Price | Latency |\n|---|---|---|\n| [Speaker A](https://shop.example/a) | 9 USD | unchecked |\n| Receiver B | unchecked | 40 ms |\n";

#[test]
fn a_product_result_names_each_unchecked_cell_each_row_without_its_page_and_no_picture() {
  let result = format!("TITLE: Speakers\n\nSECTION: Compared\n{TABLE}\n===ITEM===\nNAME: Speaker A\n");

  assert_eq!(
    gaps(&result),
    [
      "Latency of \"Speaker A\" is unchecked".to_owned(),
      "the row \"Receiver B\" names no page it is sold on".to_owned(),
      "Price of \"Receiver B\" is unchecked".to_owned(),
      "no product has a picture".to_owned(),
    ]
  );
}

#[test]
fn a_whole_product_result_and_a_result_with_no_product_entries_leave_nothing_unfilled() {
  let whole = "| Product | Price |\n|---|---|\n| [Speaker A](https://shop.example/a) | 9 USD |\n\n===ITEM===\nNAME: Speaker A\nIMAGE: https://shop.example/a.jpg :: front :: Speaker A :: front :: shop\nSOURCE [1]: https://shop.example/a :: listing\n";

  assert!(gaps(whole).is_empty(), "{:?}", gaps(whole));
  assert!(gaps(&format!("TITLE: Codecs\n\n{TABLE}")).is_empty(), "no product entries, no fill");
}
