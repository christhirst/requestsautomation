//TODO Something like this dann geht into()
impl From<refinery::OrderForm> for OrderForm {
    fn from(proto_form: refinery::OrderForm) -> Self {
        OrderForm {
            quantity: proto_form.get_quantity(),
            product_type: OilProductEnum::from(proto_form.get_product()),
        }
    }
}

// Convert from our type to the proto
impl From<OrderForm> for refinery::OrderForm {
    fn from(rust_form: OrderForm) -> Self {
        let mut order = refinery::OrderForm::new();

        order.set_quantity(rust_form.quantity);
        order.set_product(refinery::OilProductType::from(rust_form.product_type));
        order
    }
}
