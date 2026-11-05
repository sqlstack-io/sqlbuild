TEST (name "rpt_edge_case_model_keys__lists_one_key_per_model");

WITH __ref__int_edge_ratio_running_totals AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_runnin-1') AS v(check_id)
),

__ref__int_edge_ratio_averages AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('ratio_averag-1') AS v(group_code)
),

__ref__int_edge_callback_recent_offsets AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_rec-1') AS v(callback_id)
),

__ref__int_edge_callback_epoch_reads AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('callback_epo-1') AS v(check_id)
),

__ref__int_edge_callback_zone_words AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_zon-1') AS v(callback_id)
),

__ref__int_edge_callback_text_dates AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_tex-1') AS v(callback_id)
),

__ref__int_edge_key_rank_offsets AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('key_rank_off-1') AS v(check_id)
),

__ref__int_edge_key_number_reads AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('key_number_r-1') AS v(check_id)
),

__ref__int_edge_callback_epoch_parts AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_epo-1') AS v(callback_id)
),

__ref__int_edge_check_pair_filters AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_pair_f-1') AS v(check_id)
),

__ref__int_edge_check_wide_sums AS (
  SELECT
    v.key_row_id::VARCHAR AS key_row_id
  FROM VALUES
    ('check_wide_s-1') AS v(key_row_id)
),

__ref__int_edge_ratio_variant_unions AS (
  SELECT
    v.value_row_id::VARCHAR AS value_row_id
  FROM VALUES
    ('ratio_varian-1') AS v(value_row_id)
),

__ref__int_edge_check_note_unions AS (
  SELECT
    v.note_row_id::VARCHAR AS note_row_id
  FROM VALUES
    ('check_note_u-1') AS v(note_row_id)
),

__ref__int_edge_ratio_array_bounds AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('ratio_array_-1') AS v(group_code)
),

__ref__int_edge_check_zoned_literals AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_zoned_-1') AS v(check_id)
),

__ref__int_edge_callback_day_starts AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_day-1') AS v(callback_id)
),

__ref__int_edge_callback_clock_times AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_clo-1') AS v(callback_id)
),

__ref__int_edge_check_number_arrays AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_number-1') AS v(check_id)
),

__ref__int_edge_check_text_arrays AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_text_a-1') AS v(check_id)
),

__ref__int_edge_check_key_rewrites AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_key_re-1') AS v(check_id)
),

__ref__int_edge_variant_decimal_reads AS (
  SELECT
    v.value_id::VARCHAR AS value_id
  FROM VALUES
    ('variant_deci-1') AS v(value_id)
),

__ref__int_edge_ratio_safe_casts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_safe_c-1') AS v(check_id)
),

__ref__int_edge_callback_safe_dates AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_saf-1') AS v(callback_id)
),

__ref__int_edge_ratio_percent_texts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_percen-1') AS v(check_id)
),

__ref__int_edge_callback_row_numbers AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_row-1') AS v(callback_id)
),

__ref__int_edge_check_xml_checks AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_xml_ch-1') AS v(check_id)
),

__ref__int_edge_variant_xml_elements AS (
  SELECT
    v.value_id::VARCHAR AS value_id
  FROM VALUES
    ('variant_xml_-1') AS v(value_id)
),

__ref__int_edge_callback_latest_dates AS (
  SELECT
    v.merchant_zone::VARCHAR AS merchant_zone
  FROM VALUES
    ('callback_lat-1') AS v(merchant_zone)
),

__ref__int_edge_check_group_key_lists AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_group_-1') AS v(check_id)
),

__ref__int_edge_ratio_array_casts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_array_-1') AS v(check_id)
),

__ref__int_edge_ratio_text_replaces AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_text_r-1') AS v(check_id)
),

__ref__int_edge_ratio_object_reads AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_object-1') AS v(check_id)
),

__ref__int_edge_ratio_object_flags AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_object-1') AS v(check_id)
),

__ref__int_edge_ratio_identity_objects AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_identi-1') AS v(check_id)
),

__ref__int_edge_ratio_array_positions AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('ratio_array_-1') AS v(group_code)
),

__ref__int_edge_check_local_times AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_local_-1') AS v(check_id)
),

__ref__int_edge_ratio_ignore_case_reads AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_ignore-1') AS v(check_id)
),

__ref__int_edge_callback_zoned_dates AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_zon-1') AS v(callback_id)
),

__ref__int_edge_ratio_object_inserts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_object-1') AS v(check_id)
),

__ref__int_edge_callback_date_lists AS (
  SELECT
    v.merchant_zone::VARCHAR AS merchant_zone
  FROM VALUES
    ('callback_dat-1') AS v(merchant_zone)
),

__ref__int_edge_check_bracket_strips AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_bracke-1') AS v(check_id)
),

__ref__int_edge_ratio_path_checks AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_path_c-1') AS v(check_id)
),

__ref__int_edge_ratio_fixed_scales AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_fixed_-1') AS v(check_id)
),

__ref__int_edge_check_formatted_dates AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_format-1') AS v(check_id)
),

__ref__int_edge_callback_last_day AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_las-1') AS v(callback_id)
),

__ref__int_edge_check_month_starts AS (
  SELECT
    v.period_id::VARCHAR AS period_id
  FROM VALUES
    ('check_month_-1') AS v(period_id)
),

__ref__int_edge_check_key_parts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_key_pa-1') AS v(check_id)
),

__ref__int_edge_variant_expected_values AS (
  SELECT
    v.value_id::VARCHAR AS value_id
  FROM VALUES
    ('variant_expe-1') AS v(value_id)
),

__ref__int_edge_check_reduced_keys AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_reduce-1') AS v(check_id)
),

__ref__int_edge_ratio_hashes AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_hashes-1') AS v(check_id)
),

__ref__int_edge_ratio_object_differences AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_object-1') AS v(check_id)
),

__ref__int_edge_check_group_flags AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_group_-1') AS v(check_id)
),

__ref__int_edge_check_label_lookups AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_label_-1') AS v(check_id)
),

__ref__int_edge_check_distinct_tuples AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('check_distin-1') AS v(group_code)
),

__ref__int_edge_callback_start_fallbacks AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_sta-1') AS v(callback_id)
),

__ref__int_edge_amount_percent_texts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('amount_perce-1') AS v(check_id)
),

__ref__int_edge_check_minute_times AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_minute-1') AS v(check_id)
),

__ref__int_edge_check_number_labels AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_number-1') AS v(check_id)
),

__ref__int_edge_check_decoded_keys AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_decode-1') AS v(check_id)
),

__ref__int_edge_callback_london_times AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_lon-1') AS v(callback_id)
),

__ref__int_edge_check_pattern_matches AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_patter-1') AS v(check_id)
),

__ref__int_edge_check_any_amounts AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('check_any_am-1') AS v(group_code)
),

__ref__int_edge_check_sparse_arrays AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_sparse-1') AS v(check_id)
),

__ref__int_edge_check_last_known_keys AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_last_k-1') AS v(check_id)
),

__ref__int_edge_callback_day_documents AS (
  SELECT
    v.merchant_zone::VARCHAR AS merchant_zone
  FROM VALUES
    ('callback_day-1') AS v(merchant_zone)
),

__ref__int_edge_check_key_list_edits AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_key_li-1') AS v(check_id)
),

__ref__int_edge_ratio_group_objects AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('ratio_group_-1') AS v(group_code)
),

__ref__int_edge_ratio_text_forms AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_text_f-1') AS v(check_id)
),

__ref__int_edge_check_variant_branches AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_varian-1') AS v(check_id)
),

__ref__int_edge_ledger_quantity_steps AS (
  SELECT
    v.order_ref::VARCHAR AS order_ref
  FROM VALUES
    ('ledger_quant-1') AS v(order_ref)
),

__ref__int_edge_check_generated_rows AS (
  SELECT
    v.summary_id::VARCHAR AS summary_id
  FROM VALUES
    ('check_genera-1') AS v(summary_id)
),

__ref__int_edge_check_unnamed_labels AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_unname-1') AS v(check_id)
),

__ref__int_edge_basket_tag_list_reads AS (
  SELECT
    v.basket_id::VARCHAR AS basket_id
  FROM VALUES
    ('basket_tag_l-1') AS v(basket_id)
),

__ref__int_edge_basket_sku_part_filters AS (
  SELECT
    v.line_id::VARCHAR AS line_id
  FROM VALUES
    ('basket_sku_p-1') AS v(line_id)
),

__ref__int_edge_basket_distinct_tags AS (
  SELECT
    v.tag_text::VARCHAR AS tag_text
  FROM VALUES
    ('basket_disti-1') AS v(tag_text)
),

__ref__int_edge_check_amount_spreads AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('check_amount-1') AS v(group_code)
),

__ref__int_edge_check_group_pivots AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_group_-1') AS v(check_id)
),

__ref__int_edge_check_complete_pairs AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('check_comple-1') AS v(group_code)
),

__ref__int_edge_ledger_paid_quantities AS (
  SELECT
    v.order_ref::VARCHAR AS order_ref
  FROM VALUES
    ('ledger_paid_-1') AS v(order_ref)
),

__ref__int_edge_variant_element_counts AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('variant_elem-1') AS v(group_code)
),

__ref__int_edge_basket_line_lookups AS (
  SELECT
    v.line_id::VARCHAR AS line_id
  FROM VALUES
    ('basket_line_-1') AS v(line_id)
),

__ref__int_edge_basket_running_peaks AS (
  SELECT
    v.basket_id::VARCHAR AS basket_id
  FROM VALUES
    ('basket_runni-1') AS v(basket_id)
),

__ref__stg_edge_supplier_notes AS (
  SELECT
    v.note_id::VARCHAR AS note_id
  FROM VALUES
    ('supplier_not-1') AS v(note_id)
),

__ref__int_edge_clock_text_reads AS (
  SELECT
    v.text_id::VARCHAR AS text_id
  FROM VALUES
    ('clock_text_r-1') AS v(text_id)
),

__ref__int_edge_amount_bit_reads AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('amount_bit_r-1') AS v(check_id)
),

__ref__int_edge_note_patterns AS (
  SELECT
    v.note_id::VARCHAR AS note_id
  FROM VALUES
    ('note_pattern-1') AS v(note_id)
),

__ref__int_edge_ticket_tokens AS (
  SELECT
    v.line_id::VARCHAR AS line_id
  FROM VALUES
    ('ticket_token-1') AS v(line_id)
),

__ref__int_edge_basket_xml_reads AS (
  SELECT
    v.line_id::VARCHAR AS line_id
  FROM VALUES
    ('basket_xml_r-1') AS v(line_id)
),

__ref__int_edge_join_keys AS (
  SELECT
    v.row_key::VARCHAR AS row_key
  FROM VALUES
    ('join_keys-1') AS v(row_key)
),

__ref__int_edge_json_numbers AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('json_numbers-1') AS v(check_id)
),

__ref__int_edge_quantity_checks AS (
  SELECT
    v.entry_id::VARCHAR AS entry_id
  FROM VALUES
    ('quantity_che-1') AS v(entry_id)
),

__expected__rpt_edge_case_model_keys AS (
  SELECT
    'int_edge_amount_bit_reads~amount_bit_r-1' AS model_key_id,
    'int_edge_amount_bit_reads' AS model_name,
    'amount_bit_r-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_amount_percent_texts~amount_perce-1' AS model_key_id,
    'int_edge_amount_percent_texts' AS model_name,
    'amount_perce-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_basket_distinct_tags~basket_disti-1' AS model_key_id,
    'int_edge_basket_distinct_tags' AS model_name,
    'basket_disti-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_basket_line_lookups~basket_line_-1' AS model_key_id,
    'int_edge_basket_line_lookups' AS model_name,
    'basket_line_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_basket_running_peaks~basket_runni-1' AS model_key_id,
    'int_edge_basket_running_peaks' AS model_name,
    'basket_runni-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_basket_sku_part_filters~basket_sku_p-1' AS model_key_id,
    'int_edge_basket_sku_part_filters' AS model_name,
    'basket_sku_p-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_basket_tag_list_reads~basket_tag_l-1' AS model_key_id,
    'int_edge_basket_tag_list_reads' AS model_name,
    'basket_tag_l-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_basket_xml_reads~basket_xml_r-1' AS model_key_id,
    'int_edge_basket_xml_reads' AS model_name,
    'basket_xml_r-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_clock_times~callback_clo-1' AS model_key_id,
    'int_edge_callback_clock_times' AS model_name,
    'callback_clo-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_date_lists~callback_dat-1' AS model_key_id,
    'int_edge_callback_date_lists' AS model_name,
    'callback_dat-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_day_documents~callback_day-1' AS model_key_id,
    'int_edge_callback_day_documents' AS model_name,
    'callback_day-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_day_starts~callback_day-1' AS model_key_id,
    'int_edge_callback_day_starts' AS model_name,
    'callback_day-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_epoch_parts~callback_epo-1' AS model_key_id,
    'int_edge_callback_epoch_parts' AS model_name,
    'callback_epo-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_epoch_reads~callback_epo-1' AS model_key_id,
    'int_edge_callback_epoch_reads' AS model_name,
    'callback_epo-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_last_day~callback_las-1' AS model_key_id,
    'int_edge_callback_last_day' AS model_name,
    'callback_las-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_latest_dates~callback_lat-1' AS model_key_id,
    'int_edge_callback_latest_dates' AS model_name,
    'callback_lat-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_london_times~callback_lon-1' AS model_key_id,
    'int_edge_callback_london_times' AS model_name,
    'callback_lon-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_recent_offsets~callback_rec-1' AS model_key_id,
    'int_edge_callback_recent_offsets' AS model_name,
    'callback_rec-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_row_numbers~callback_row-1' AS model_key_id,
    'int_edge_callback_row_numbers' AS model_name,
    'callback_row-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_safe_dates~callback_saf-1' AS model_key_id,
    'int_edge_callback_safe_dates' AS model_name,
    'callback_saf-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_start_fallbacks~callback_sta-1' AS model_key_id,
    'int_edge_callback_start_fallbacks' AS model_name,
    'callback_sta-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_text_dates~callback_tex-1' AS model_key_id,
    'int_edge_callback_text_dates' AS model_name,
    'callback_tex-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_zone_words~callback_zon-1' AS model_key_id,
    'int_edge_callback_zone_words' AS model_name,
    'callback_zon-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_callback_zoned_dates~callback_zon-1' AS model_key_id,
    'int_edge_callback_zoned_dates' AS model_name,
    'callback_zon-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_amount_spreads~check_amount-1' AS model_key_id,
    'int_edge_check_amount_spreads' AS model_name,
    'check_amount-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_any_amounts~check_any_am-1' AS model_key_id,
    'int_edge_check_any_amounts' AS model_name,
    'check_any_am-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_bracket_strips~check_bracke-1' AS model_key_id,
    'int_edge_check_bracket_strips' AS model_name,
    'check_bracke-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_complete_pairs~check_comple-1' AS model_key_id,
    'int_edge_check_complete_pairs' AS model_name,
    'check_comple-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_decoded_keys~check_decode-1' AS model_key_id,
    'int_edge_check_decoded_keys' AS model_name,
    'check_decode-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_distinct_tuples~check_distin-1' AS model_key_id,
    'int_edge_check_distinct_tuples' AS model_name,
    'check_distin-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_formatted_dates~check_format-1' AS model_key_id,
    'int_edge_check_formatted_dates' AS model_name,
    'check_format-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_generated_rows~check_genera-1' AS model_key_id,
    'int_edge_check_generated_rows' AS model_name,
    'check_genera-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_group_flags~check_group_-1' AS model_key_id,
    'int_edge_check_group_flags' AS model_name,
    'check_group_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_group_key_lists~check_group_-1' AS model_key_id,
    'int_edge_check_group_key_lists' AS model_name,
    'check_group_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_group_pivots~check_group_-1' AS model_key_id,
    'int_edge_check_group_pivots' AS model_name,
    'check_group_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_key_list_edits~check_key_li-1' AS model_key_id,
    'int_edge_check_key_list_edits' AS model_name,
    'check_key_li-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_key_parts~check_key_pa-1' AS model_key_id,
    'int_edge_check_key_parts' AS model_name,
    'check_key_pa-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_key_rewrites~check_key_re-1' AS model_key_id,
    'int_edge_check_key_rewrites' AS model_name,
    'check_key_re-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_label_lookups~check_label_-1' AS model_key_id,
    'int_edge_check_label_lookups' AS model_name,
    'check_label_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_last_known_keys~check_last_k-1' AS model_key_id,
    'int_edge_check_last_known_keys' AS model_name,
    'check_last_k-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_local_times~check_local_-1' AS model_key_id,
    'int_edge_check_local_times' AS model_name,
    'check_local_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_minute_times~check_minute-1' AS model_key_id,
    'int_edge_check_minute_times' AS model_name,
    'check_minute-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_month_starts~check_month_-1' AS model_key_id,
    'int_edge_check_month_starts' AS model_name,
    'check_month_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_note_unions~check_note_u-1' AS model_key_id,
    'int_edge_check_note_unions' AS model_name,
    'check_note_u-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_number_arrays~check_number-1' AS model_key_id,
    'int_edge_check_number_arrays' AS model_name,
    'check_number-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_number_labels~check_number-1' AS model_key_id,
    'int_edge_check_number_labels' AS model_name,
    'check_number-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_pair_filters~check_pair_f-1' AS model_key_id,
    'int_edge_check_pair_filters' AS model_name,
    'check_pair_f-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_pattern_matches~check_patter-1' AS model_key_id,
    'int_edge_check_pattern_matches' AS model_name,
    'check_patter-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_reduced_keys~check_reduce-1' AS model_key_id,
    'int_edge_check_reduced_keys' AS model_name,
    'check_reduce-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_sparse_arrays~check_sparse-1' AS model_key_id,
    'int_edge_check_sparse_arrays' AS model_name,
    'check_sparse-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_text_arrays~check_text_a-1' AS model_key_id,
    'int_edge_check_text_arrays' AS model_name,
    'check_text_a-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_unnamed_labels~check_unname-1' AS model_key_id,
    'int_edge_check_unnamed_labels' AS model_name,
    'check_unname-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_variant_branches~check_varian-1' AS model_key_id,
    'int_edge_check_variant_branches' AS model_name,
    'check_varian-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_wide_sums~check_wide_s-1' AS model_key_id,
    'int_edge_check_wide_sums' AS model_name,
    'check_wide_s-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_xml_checks~check_xml_ch-1' AS model_key_id,
    'int_edge_check_xml_checks' AS model_name,
    'check_xml_ch-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_check_zoned_literals~check_zoned_-1' AS model_key_id,
    'int_edge_check_zoned_literals' AS model_name,
    'check_zoned_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_clock_text_reads~clock_text_r-1' AS model_key_id,
    'int_edge_clock_text_reads' AS model_name,
    'clock_text_r-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_join_keys~join_keys-1' AS model_key_id,
    'int_edge_join_keys' AS model_name,
    'join_keys-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_json_numbers~json_numbers-1' AS model_key_id,
    'int_edge_json_numbers' AS model_name,
    'json_numbers-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_key_number_reads~key_number_r-1' AS model_key_id,
    'int_edge_key_number_reads' AS model_name,
    'key_number_r-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_key_rank_offsets~key_rank_off-1' AS model_key_id,
    'int_edge_key_rank_offsets' AS model_name,
    'key_rank_off-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ledger_paid_quantities~ledger_paid_-1' AS model_key_id,
    'int_edge_ledger_paid_quantities' AS model_name,
    'ledger_paid_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ledger_quantity_steps~ledger_quant-1' AS model_key_id,
    'int_edge_ledger_quantity_steps' AS model_name,
    'ledger_quant-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_note_patterns~note_pattern-1' AS model_key_id,
    'int_edge_note_patterns' AS model_name,
    'note_pattern-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_quantity_checks~quantity_che-1' AS model_key_id,
    'int_edge_quantity_checks' AS model_name,
    'quantity_che-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_array_bounds~ratio_array_-1' AS model_key_id,
    'int_edge_ratio_array_bounds' AS model_name,
    'ratio_array_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_array_casts~ratio_array_-1' AS model_key_id,
    'int_edge_ratio_array_casts' AS model_name,
    'ratio_array_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_array_positions~ratio_array_-1' AS model_key_id,
    'int_edge_ratio_array_positions' AS model_name,
    'ratio_array_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_averages~ratio_averag-1' AS model_key_id,
    'int_edge_ratio_averages' AS model_name,
    'ratio_averag-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_fixed_scales~ratio_fixed_-1' AS model_key_id,
    'int_edge_ratio_fixed_scales' AS model_name,
    'ratio_fixed_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_group_objects~ratio_group_-1' AS model_key_id,
    'int_edge_ratio_group_objects' AS model_name,
    'ratio_group_-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_hashes~ratio_hashes-1' AS model_key_id,
    'int_edge_ratio_hashes' AS model_name,
    'ratio_hashes-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_identity_objects~ratio_identi-1' AS model_key_id,
    'int_edge_ratio_identity_objects' AS model_name,
    'ratio_identi-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_ignore_case_reads~ratio_ignore-1' AS model_key_id,
    'int_edge_ratio_ignore_case_reads' AS model_name,
    'ratio_ignore-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_object_differences~ratio_object-1' AS model_key_id,
    'int_edge_ratio_object_differences' AS model_name,
    'ratio_object-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_object_flags~ratio_object-1' AS model_key_id,
    'int_edge_ratio_object_flags' AS model_name,
    'ratio_object-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_object_inserts~ratio_object-1' AS model_key_id,
    'int_edge_ratio_object_inserts' AS model_name,
    'ratio_object-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_object_reads~ratio_object-1' AS model_key_id,
    'int_edge_ratio_object_reads' AS model_name,
    'ratio_object-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_path_checks~ratio_path_c-1' AS model_key_id,
    'int_edge_ratio_path_checks' AS model_name,
    'ratio_path_c-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_percent_texts~ratio_percen-1' AS model_key_id,
    'int_edge_ratio_percent_texts' AS model_name,
    'ratio_percen-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_running_totals~ratio_runnin-1' AS model_key_id,
    'int_edge_ratio_running_totals' AS model_name,
    'ratio_runnin-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_safe_casts~ratio_safe_c-1' AS model_key_id,
    'int_edge_ratio_safe_casts' AS model_name,
    'ratio_safe_c-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_text_forms~ratio_text_f-1' AS model_key_id,
    'int_edge_ratio_text_forms' AS model_name,
    'ratio_text_f-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_text_replaces~ratio_text_r-1' AS model_key_id,
    'int_edge_ratio_text_replaces' AS model_name,
    'ratio_text_r-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ratio_variant_unions~ratio_varian-1' AS model_key_id,
    'int_edge_ratio_variant_unions' AS model_name,
    'ratio_varian-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_ticket_tokens~ticket_token-1' AS model_key_id,
    'int_edge_ticket_tokens' AS model_name,
    'ticket_token-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_variant_decimal_reads~variant_deci-1' AS model_key_id,
    'int_edge_variant_decimal_reads' AS model_name,
    'variant_deci-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_variant_element_counts~variant_elem-1' AS model_key_id,
    'int_edge_variant_element_counts' AS model_name,
    'variant_elem-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_variant_expected_values~variant_expe-1' AS model_key_id,
    'int_edge_variant_expected_values' AS model_name,
    'variant_expe-1' AS model_key
  UNION ALL
  SELECT
    'int_edge_variant_xml_elements~variant_xml_-1' AS model_key_id,
    'int_edge_variant_xml_elements' AS model_name,
    'variant_xml_-1' AS model_key
  UNION ALL
  SELECT
    'stg_edge_supplier_notes~supplier_not-1' AS model_key_id,
    'stg_edge_supplier_notes' AS model_name,
    'supplier_not-1' AS model_key
)
