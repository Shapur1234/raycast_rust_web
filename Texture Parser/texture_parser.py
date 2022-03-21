from ctypes import resize
from enum import unique
from PIL import Image
import numpy

img = Image.open("Texture Parser/wall_brick.png")
numpydata = numpy.asarray(img)

result =[[]];
for key, line in enumerate(numpydata):
    result.append([]);
    for color in line:
        result[key] += f"Color::new({color[0]}, {color[1]}, {color[2]})\n"
        
with open("Texture Parser/texture_output.txt", "w") as file:
    for i in result:
        file.write("vec![\n")
        for a in i:
            file.write(a)
        file.writelines("\n],\n")

print(str(result[0]))

# uniques = list(all)