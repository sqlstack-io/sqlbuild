FUNCTION (
  description 'Advance an order quantity in the inherited function namespace.',
  tags [edge_case],
  arguments (quantity INTEGER),
  returns INTEGER,
);

-- One more than the quantity it is given.
quantity + 1
