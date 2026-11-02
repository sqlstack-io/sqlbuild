MODEL (
  description """
    One row per key of each edge-case model that reads a construct translation tools refuse, so
    that every such model has a downstream reader whose own SQL translates
  """,
  tags [edge_case],
  columns (
    model_key_id (type VARCHAR, nullable false, audits [not_null, unique]),
    model_name (type VARCHAR),
    model_key (type VARCHAR),
  ),
);

WITH model_keys AS (
  SELECT 'int_edge_ratio_running_totals' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_running_totals") AS r
  UNION ALL
  SELECT 'int_edge_ratio_averages' AS model_name, r.group_code::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_averages") AS r
  UNION ALL
  SELECT 'int_edge_callback_recent_offsets' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_recent_offsets") AS r
  UNION ALL
  SELECT 'int_edge_callback_epoch_reads' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_epoch_reads") AS r
  UNION ALL
  SELECT 'int_edge_callback_zone_words' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_zone_words") AS r
  UNION ALL
  SELECT 'int_edge_callback_text_dates' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_text_dates") AS r
  UNION ALL
  SELECT 'int_edge_key_rank_offsets' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_key_rank_offsets") AS r
  UNION ALL
  SELECT 'int_edge_key_number_reads' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_key_number_reads") AS r
  UNION ALL
  SELECT 'int_edge_callback_epoch_parts' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_epoch_parts") AS r
  UNION ALL
  SELECT 'int_edge_check_pair_filters' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_pair_filters") AS r
  UNION ALL
  SELECT 'int_edge_check_wide_sums' AS model_name, r.key_row_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_wide_sums") AS r
  UNION ALL
  SELECT 'int_edge_ratio_variant_unions' AS model_name, r.value_row_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_variant_unions") AS r
  UNION ALL
  SELECT 'int_edge_check_note_unions' AS model_name, r.note_row_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_note_unions") AS r
  UNION ALL
  SELECT 'int_edge_ratio_array_bounds' AS model_name, r.group_code::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_array_bounds") AS r
  UNION ALL
  SELECT 'int_edge_check_zoned_literals' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_zoned_literals") AS r
  UNION ALL
  SELECT 'int_edge_callback_day_starts' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_day_starts") AS r
  UNION ALL
  SELECT 'int_edge_callback_clock_times' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_clock_times") AS r
  UNION ALL
  SELECT 'int_edge_check_key_rewrites' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_key_rewrites") AS r
  UNION ALL
  SELECT 'int_edge_variant_decimal_reads' AS model_name, r.value_id::VARCHAR AS model_key
  FROM __ref("int_edge_variant_decimal_reads") AS r
  UNION ALL
  SELECT 'int_edge_ratio_safe_casts' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_safe_casts") AS r
  UNION ALL
  SELECT 'int_edge_callback_safe_dates' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_safe_dates") AS r
  UNION ALL
  SELECT 'int_edge_ratio_percent_texts' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_percent_texts") AS r
  UNION ALL
  SELECT 'int_edge_callback_row_numbers' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_row_numbers") AS r
  UNION ALL
  SELECT 'int_edge_check_xml_checks' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_xml_checks") AS r
  UNION ALL
  SELECT 'int_edge_variant_xml_elements' AS model_name, r.value_id::VARCHAR AS model_key
  FROM __ref("int_edge_variant_xml_elements") AS r
  UNION ALL
  SELECT 'int_edge_callback_latest_dates' AS model_name, r.merchant_zone::VARCHAR AS model_key
  FROM __ref("int_edge_callback_latest_dates") AS r
  UNION ALL
  SELECT 'int_edge_check_group_key_lists' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_group_key_lists") AS r
  UNION ALL
  SELECT 'int_edge_ratio_array_casts' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_array_casts") AS r
  UNION ALL
  SELECT 'int_edge_ratio_text_replaces' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_text_replaces") AS r
  UNION ALL
  SELECT 'int_edge_ratio_object_reads' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_object_reads") AS r
  UNION ALL
  SELECT 'int_edge_ratio_object_flags' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_object_flags") AS r
  UNION ALL
  SELECT 'int_edge_ratio_identity_objects' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_identity_objects") AS r
  UNION ALL
  SELECT 'int_edge_ratio_array_positions' AS model_name, r.group_code::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_array_positions") AS r
  UNION ALL
  SELECT 'int_edge_check_local_times' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_local_times") AS r
  UNION ALL
  SELECT 'int_edge_ratio_ignore_case_reads' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_ignore_case_reads") AS r
  UNION ALL
  SELECT 'int_edge_callback_zoned_dates' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_zoned_dates") AS r
  UNION ALL
  SELECT 'int_edge_ratio_object_inserts' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_object_inserts") AS r
  UNION ALL
  SELECT 'int_edge_callback_date_lists' AS model_name, r.merchant_zone::VARCHAR AS model_key
  FROM __ref("int_edge_callback_date_lists") AS r
  UNION ALL
  SELECT 'int_edge_check_bracket_strips' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_bracket_strips") AS r
  UNION ALL
  SELECT 'int_edge_ratio_path_checks' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_path_checks") AS r
  UNION ALL
  SELECT 'int_edge_ratio_fixed_scales' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_fixed_scales") AS r
  UNION ALL
  SELECT 'int_edge_check_formatted_dates' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_formatted_dates") AS r
  UNION ALL
  SELECT 'int_edge_callback_last_day' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_last_day") AS r
  UNION ALL
  SELECT 'int_edge_check_month_starts' AS model_name, r.period_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_month_starts") AS r
  UNION ALL
  SELECT 'int_edge_variant_expected_values' AS model_name, r.value_id::VARCHAR AS model_key
  FROM __ref("int_edge_variant_expected_values") AS r
  UNION ALL
  SELECT 'int_edge_check_reduced_keys' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_reduced_keys") AS r
  UNION ALL
  SELECT 'int_edge_ratio_hashes' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_hashes") AS r
  UNION ALL
  SELECT 'int_edge_ratio_object_differences' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_object_differences") AS r
  UNION ALL
  SELECT 'int_edge_check_group_flags' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_group_flags") AS r
  UNION ALL
  SELECT 'int_edge_check_label_lookups' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_label_lookups") AS r
  UNION ALL
  SELECT 'int_edge_check_distinct_tuples' AS model_name, r.group_code::VARCHAR AS model_key
  FROM __ref("int_edge_check_distinct_tuples") AS r
  UNION ALL
  SELECT 'int_edge_callback_start_fallbacks' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_start_fallbacks") AS r
  UNION ALL
  SELECT 'int_edge_amount_percent_texts' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_amount_percent_texts") AS r
  UNION ALL
  SELECT 'int_edge_check_minute_times' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_minute_times") AS r
  UNION ALL
  SELECT 'int_edge_check_number_labels' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_number_labels") AS r
  UNION ALL
  SELECT 'int_edge_check_decoded_keys' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_decoded_keys") AS r
  UNION ALL
  SELECT 'int_edge_callback_london_times' AS model_name, r.callback_id::VARCHAR AS model_key
  FROM __ref("int_edge_callback_london_times") AS r
  UNION ALL
  SELECT 'int_edge_check_pattern_matches' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_pattern_matches") AS r
  UNION ALL
  SELECT 'int_edge_check_any_amounts' AS model_name, r.group_code::VARCHAR AS model_key
  FROM __ref("int_edge_check_any_amounts") AS r
  UNION ALL
  SELECT 'int_edge_check_sparse_arrays' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_sparse_arrays") AS r
  UNION ALL
  SELECT 'int_edge_check_last_known_keys' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_last_known_keys") AS r
  UNION ALL
  SELECT 'int_edge_callback_day_documents' AS model_name, r.merchant_zone::VARCHAR AS model_key
  FROM __ref("int_edge_callback_day_documents") AS r
  UNION ALL
  SELECT 'int_edge_check_key_list_edits' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_key_list_edits") AS r
  UNION ALL
  SELECT 'int_edge_ratio_group_objects' AS model_name, r.group_code::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_group_objects") AS r
  UNION ALL
  SELECT 'int_edge_ratio_text_forms' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_ratio_text_forms") AS r
  UNION ALL
  SELECT 'int_edge_check_variant_branches' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_variant_branches") AS r
  UNION ALL
  SELECT 'int_edge_ledger_quantity_steps' AS model_name, r.order_ref::VARCHAR AS model_key
  FROM __ref("int_edge_ledger_quantity_steps") AS r
  UNION ALL
  SELECT 'int_edge_check_generated_rows' AS model_name, r.summary_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_generated_rows") AS r
  UNION ALL
  SELECT 'int_edge_check_unnamed_labels' AS model_name, r.check_id::VARCHAR AS model_key
  FROM __ref("int_edge_check_unnamed_labels") AS r
),

final AS (
  SELECT
    k.model_name || '~' || k.model_key AS model_key_id,
    k.model_name,
    k.model_key
  FROM model_keys AS k
)

SELECT *
FROM final
