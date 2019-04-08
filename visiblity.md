private = private, only usable with in the same module (default)
protected = usable to other modules in the same directory and the children of the current directory
internal = usable to all other modules in the current package
public = usable by anyone in any dependent packages

Testing files are located in a separate directory that should have the same structure as the main directory,
and use the same names plus 'Test' at the end. Test files will have the same access as the file they are named after, so if there
is a module named Utils::StringUtils then it's test file is in the test directory and is named Utils::StringUtilsTest.
It will have access to all the private functions and data that the main Utils::StringUtils has. All published functions
in a test file are considered tests and will be used as entry points.

Any directory that has a module of the same name will use that file to represent the directory as a module on it's own.
For example say there is a directory named List with a file in it named List.let. Another module can use the contents
of List.let with the name List, and does not need to use List::List. This allows any single file module X to be refactored
into many separate files under a directory named X and use X.let inside the directory to publish files from other modules
and keep the same api surface, while hiding those other files by having them not publish anything.

If module A defines or reexports no functions or types visible to module B and has no submodules that do the same,
then module B cannot import or use module A, and editors and tools should hide the existence of module A from B.

A module can import things from the same scope or child scopes using relative names. For example if directory A
contains module B and C, then C can have 'import { someFun } from B;' without needing the full name. For anything else
in the same package, the full package path must be used. So if directory Iterables contains List and Set, and List
contains Partition and set contains Set contains SetHelpers, if SetHelpers wants to use something inside Partition it
must import from 'Iterables::List::Partition' and NOT from 'List::Partition' even though both are children of the
'Iterables' module.

If importing something from a different package, the full package name must be used. So if a module in a different
package wanted to use List, it would need to 'import com.github.coolCollections::coolCollections::Iterables::List';
While this seems verbose, it makes it very explicit where an import is coming from and means you don't need to worry about
organization or package names inside a package and ides and tooling should make this easier.

example app

.git
src
  main
    List
      Partition.let
        protected fun partition() ...
      GroupBy.let
        protected fun groupBy() ...
      List.let
        internal partition from Partition
        internal groupBy   from GroupBy
    Utils
      StringUtils.let
        internal fun join() ...
      NumberUtils.let
        internal fun sum() ...
    Test.let
      import { groupBy, partition } from List
      import { join } from Utils::StringUtils
      import { sum } from Utils::NumberUtils
      public doThing() ...
  test
    List
      Partition.let
        fun testPartition()
    Utils
      StringUtilsTest
        StringUtilsHelper.let
          protect fun helpTestJoin() ...
        StringUtilsTest.let
          import { helpTestJoin } from StringUtilsHelper
          publish fun testJoin() ...
build.file


```
// String and Int are implicitly imported
import Core.Number;

fun compose[Input, Output, Middle <: HList](first: {Input -> Middle}, second: {Middle -> Output}): Output = { in: Input => second.apply(first(in) }

fun add(left: Int, right: Int): Int = left + right

fun parseIntPair(in: String): HList[Int, Int] = {
  // `in.split(',')` is the same as calling `String.split(in, ',')`
  // this is because String is a module that defines a type named String and a function named split that accepts
  // a String as it's first argument.
  // String is already imported

  let List[leftStr, rightStr] = in.split(',');
  let left = Number::parseInt(leftStr);
  let right = Number::parseInt(rightStr);
  return HList(left, right);
}

fun double(num: Int): Int = num * 2

fun main() = {
  let raw = '4,5';
  // Input is String, Output is Int and Middle is HList[Int, Int]
  let firstHalf = compose(parseIntPair, add);

  // Input is String, Output is In and Middle is HList[Int]
  let secondHalf = compose(firstHalf, double);

  let result = secondHalf(raw);
  Test.assert(18, result);
}
```

```
class Person [
  name: String;
  age: Float;
]

fun test(): List[Int] = {
  let list = List('b', 'a', 'b')
  let map = Map['a': 1, 'b': 2]
  
  list.map({ letter: String -> Int => map.get(letter) })
}
```







