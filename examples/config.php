<?php

require __DIR__ . '/../vendor/autoload.php';

use Vipayment\Vipayment;

// Load environment variables or set credentials here
// You can also use a .env file if you prefer, but for simplicity we use variables here.

$apiId = 'your_api_id';
$apiKey = 'your_api_key';

// Return the configured client instance
return new Vipayment($apiId, $apiKey);
