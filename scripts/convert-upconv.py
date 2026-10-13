#!/usr/bin/env python3
import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

parser = argparse.ArgumentParser(description='Convert original waifu2x upconv7 JSON weights to ONNX')
parser.add_argument('weights', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
layers = json.loads(args.weights.read_text())
assert len(layers) == 7
assert layers[0]['model_config']['arch_name'] == 'upconv_7'
nodes, initializers = [], []
previous = 'input'
for index, layer in enumerate(layers):
    transposed = layer['class_name'] == 'nn.SpatialFullConvolution'
    assert layer['class_name'] in ('nn.SpatialFullConvolution', 'nn.SpatialConvolutionMM')
    weight = np.asarray(layer['weight'], dtype=np.float32)
    bias = np.asarray(layer['bias'], dtype=np.float32)
    inputs = [previous, f'w{index}']
    initializers.append(numpy_helper.from_array(weight, inputs[1]))
    if not transposed:
        inputs.append(f'b{index}')
        initializers.append(numpy_helper.from_array(bias, inputs[2]))
    else:
        assert np.count_nonzero(bias) == 0
    output = f'conv{index}'
    nodes.append(helper.make_node('ConvTranspose' if transposed else 'Conv', inputs, [output],
                                  kernel_shape=[layer['kH'], layer['kW']],
                                  strides=[layer['dH'], layer['dW']],
                                  pads=[layer['padH'], layer['padW']] * 2))
    if not transposed:
        previous = f'activation{index}'
        nodes.append(helper.make_node('LeakyRelu', [output], [previous], alpha=0.1))
    else:
        previous = output
for name, value in [('minimum', 0.0), ('maximum', 1.0)]:
    initializers.append(numpy_helper.from_array(np.array(value, dtype=np.float32), name))
nodes.append(helper.make_node('Clip', [previous, 'minimum', 'maximum'], ['output']))
graph = helper.make_graph(nodes, 'waifu2x_upconv7',
                          [helper.make_tensor_value_info('input', TensorProto.FLOAT, [1, 3, 'height', 'width'])],
                          [helper.make_tensor_value_info('output', TensorProto.FLOAT, [1, 3, 'out_height', 'out_width'])],
                          initializers)
model = helper.make_model(graph, producer_name='suwa', opset_imports=[helper.make_opsetid('', 17)], ir_version=9)
onnx.checker.check_model(model)
args.output.parent.mkdir(parents=True, exist_ok=True)
onnx.save(model, args.output)
manifest = dict(name='waifu2x UpConv7 · Fast 2×', file=args.output.name,
                sha256=hashlib.sha256(args.output.read_bytes()).hexdigest(), scale=2, tile=256, padding=14)
args.output.with_suffix('.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f'Wrote {args.output} ({args.output.stat().st_size} bytes)')
print('Source SHA-256:', hashlib.sha256(args.weights.read_bytes()).hexdigest())
